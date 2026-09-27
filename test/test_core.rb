# frozen_string_literal: true
require 'minitest/autorun'
require 'tmpdir'
require_relative '../lib/live2d_studio'

class TrackingTest < Minitest::Test
  def schema
    [
      { 'id' => 'ParamAngleX', 'min' => -30, 'max' => 30, 'default' => 0 },
      { 'id' => 'ParamEyeLOpen', 'min' => 0, 'max' => 1.4, 'default' => 1 },
      { 'id' => 'ParamEyeROpen', 'min' => 0, 'max' => 1.4, 'default' => 1 },
      { 'id' => 'ParamMouthOpenY', 'min' => 0, 'max' => 1, 'default' => 0 },
      { 'id' => 'ParamMouthForm', 'min' => -1, 'max' => 1, 'default' => 0 }
    ]
  end

  def test_all_52_arkit_channels_survive_json_and_pipe
    assert_equal 52, Live2D::Tracking::ARKIT.uniq.size
    raw = Live2D::Tracking::ARKIT.to_h { |key| [key, 0.25] }
    assert_equal raw, Live2D::Tracking::Parser.parse(JSON.generate(blendshapes: raw))
    assert_equal raw, Live2D::Tracking::Parser.parse(raw.keys.map { |key| "#{key}-25" }.join('|'))
  end
  def test_negative_head_rotation_and_unknown_fields
    frame = Live2D::Tracking::Parser.parse('=head#-12.5,24,-3,0,0,0|jawOpen-40|unknown-100')
    assert_equal(-12.5, frame['pitch'])
    assert_equal 24.0, frame['yaw']
    assert_in_delta 0.4, frame['jawOpen']
    refute frame.key?('unknown')
  end
  def test_ifacialmocap_left_right_aliases_and_v2_separator
    frame = Live2D::Tracking::Parser.parse('eyeBlink_L-5|mouthSmile_R&80|browDown_L-20')
    assert_in_delta 0.05, frame['eyeBlinkLeft']
    assert_in_delta 0.8, frame['mouthSmileRight']
    assert_in_delta 0.2, frame['browDownLeft']
  end
  def test_invalid_packets_rejected
    ['{', '{"jawOpen":null}', '{"jawOpen":2}', '{"yaw":1e309}', 'x' * 65_537].each do |packet|
      assert_raises(Live2D::Error) { Live2D::Tracking::Parser.parse(packet) }
    end
  end
  def test_mapper_clamps_to_model_and_falls_back_from_funnel
    mapper = Live2D::Tracking::Mapper.new(schema)
    values = mapper.map('yaw' => 70, 'mouthFunnel' => 1, 'jawOpen' => 0.7, 'eyeBlinkLeft' => 0.02)
    assert_equal 30, values['ParamAngleX']
    assert_in_delta(-0.7, values['ParamMouthForm'])
    assert_in_delta 0.98, values['ParamEyeLOpen']
    refute values.key?('ParamMouthFunnel')
  end
  def test_advanced_mapping_does_not_double_apply_fallback
    mapper = Live2D::Tracking::Mapper.new(schema + [{ 'id' => 'ParamMouthFunnel', 'min' => 0, 'max' => 1, 'default' => 0 }])
    values = mapper.map('mouthFunnel' => 0.8)
    assert_equal 0, values['ParamMouthForm']
    assert_in_delta 0.8, values['ParamMouthFunnel']
    assert_in_delta 0.9, mapper.map('ParamMouthFunnel' => 0.9)['ParamMouthFunnel']
  end
  def test_calibration_robust_neutral_and_range
    calibration = Live2D::Tracking::Calibration.new(samples: 3)
    [12, 80, 10].each { |v| calibration.apply('yaw' => v, 'jawOpen' => 0.1) }
    assert calibration.ready?
    assert_in_delta 0, calibration.apply('yaw' => 12)['yaw']
    assert_in_delta 1, calibration.apply('jawOpen' => 1)['jawOpen']
    calibration.reset
    refute calibration.ready?
  end
  def test_extended_only_inputs_fall_back_to_standard_rig
    mapper = Live2D::Tracking::Mapper.new(schema + [{ 'id' => 'ParamBrowLY', 'min' => -1, 'max' => 1, 'default' => 0 }])
    values = mapper.map('ParamEyeSquint' => 1, 'ParamMouthCornerRound' => 1, 'ParamBrowDepth' => 1)
    assert_in_delta 0.65, values['ParamEyeLOpen']
    assert_in_delta(-0.6, values['ParamMouthForm'])
    assert_in_delta(-0.5, values['ParamBrowLY'])
  end
  def test_engine_staleness_and_direct_parameter_expiry
    engine = Live2D::Engine.new('mode' => 'agent', 'stale_after' => 0.2, 'smoothing_seconds' => 0.001)
    mapper = Live2D::Tracking::Mapper.new(schema)
    engine.ingest('agent', { 'yaw' => 20 }, 0)
    assert_in_delta 20, engine.sample(mapper, 0, 0.1)['ParamAngleX']
    assert_in_delta 0, engine.sample(mapper, 1, 0.1)['ParamAngleX']
    engine.parameters({ 'ParamAngleX' => -50 }, 2)
    assert_in_delta(-30, engine.sample(mapper, 1.2, 0.1)['ParamAngleX'])
    assert_in_delta 0, engine.sample(mapper, 3, 0.1)['ParamAngleX']
  end
  def test_fixed_timeline_deterministic_and_stable_order
    Dir.mktmpdir do |dir|
      path = File.join(dir, 'timeline.json')
      File.write(path, JSON.generate(duration: 2, events: [
        { time: 1, command: { op: 'a' } }, { time: 0, command: { op: 'b' } }, { time: 1, command: { op: 'c' } }
      ]))
      a, b = Live2D::Timeline.new(path), Live2D::Timeline.new(path)
      frames_a = 120.times.map { |i| a.due(i / 60.0) }
      frames_b = 120.times.map { |i| b.due(i / 60.0) }
      assert_equal frames_a, frames_b
      assert_equal %w[a c], frames_a[60].map { |v| v['op'] }
    end
  end
end

class NetworkTest < Minitest::Test
  def setup
    @inbox = Live2D::Inbox.new
    @port = TCPServer.open('127.0.0.1', 0) { |s| s.addr[1] }
    @udp_port = UDPSocket.open { |s| s.bind('127.0.0.1', 0); s.addr[1] }
    @network = Live2D::Network.new({ 'tracking' => { 'bind' => '127.0.0.1', 'udp_ports' => [@udp_port] },
      'api' => { 'bind' => '127.0.0.1', 'port' => @port, 'max_clients' => 4, 'max_packet_bytes' => 1024 } }, @inbox, token: 'test-token').start
  end
  def teardown = @network.stop
  def test_authentication_and_fragmented_tcp_requests
    socket = TCPSocket.new('127.0.0.1', @port)
    socket.write("{\"token\":\"wrong\"}\n")
    assert IO.select([socket], nil, nil, 2)
    assert_equal false, JSON.parse(socket.gets)['ok']
    socket.write('{"token":"test-')
    socket.write("token\",\"op\":\"status\",\"id\":3}\n")
    command = nil
    Timeout.timeout(3) do
      loop do
        _, commands = @inbox.drain
        unless commands.empty?
          command, reply = commands.first
          reply << { ok: true, result: { native: true } }
          break
        end
        sleep 0.01
      end
    end
    assert_equal 'status', command['op']
    assert IO.select([socket], nil, nil, 2)
    assert_equal 3, JSON.parse(socket.gets)['id']
  ensure
    socket&.close
  end
  def test_udp_listener_survives_malformed_packet
    socket = UDPSocket.new
    socket.send('{invalid', 0, '127.0.0.1', @udp_port)
    socket.send('jawOpen-50|eyeBlinkLeft-0', 0, '127.0.0.1', @udp_port)
    received = nil
    Timeout.timeout(3) do
      loop do
        frames, = @inbox.drain
        if frames['phone']
          received = frames['phone']; break
        end
        sleep 0.01
      end
    end
    assert_in_delta 0.5, received['jawOpen']
  ensure
    socket&.close
  end
  def test_queue_is_bounded_and_tracking_is_coalesced
    128.times { @inbox.push('op' => 'status') }
    assert_raises(Live2D::Error) { @inbox.push('op' => 'status') }
    100.times { |i| @inbox.tracking('phone', { 'yaw' => i }) }
    frames, commands = @inbox.drain
    assert_equal 99, frames['phone']['yaw']
    assert_equal 128, commands.size
  end
end
