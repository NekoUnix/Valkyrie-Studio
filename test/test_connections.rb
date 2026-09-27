require 'minitest/autorun'
require_relative '../lib/live2d_studio'

class ConnectionsTest < Minitest::Test
  def test_phone_request_and_live_udp_status
    inbox = Live2D::Inbox.new
    config = Live2D.config
    config['tracking']['udp_ports'] = []
    config['api']['port'] = 0
    network = Live2D::Network.new(config, inbox).start
    receiver = UDPSocket.new
    receiver.bind('127.0.0.1', 0)
    probe = UDPSocket.new
    probe.bind('127.0.0.1', 0)
    port = probe.addr[1]
    probe.close
    connections = Live2D::Connections.new(network, {})
    connections.configure('phone_ip' => '127.0.0.1', 'phone_port' => receiver.addr[1],
      'listen_bind' => '127.0.0.1', 'listen_port' => port, 'webcam_port' => port)
    connections.start('phone')
    assert IO.select([receiver], nil, nil, 2)
    assert_match(/iFacialMocap_/, receiver.recv(1024))
    receiver.send('jawOpen-40|eyeBlink_L-10', 0, '127.0.0.1', port)
    Timeout.timeout(3) { sleep 0.01 until connections.status.dig('phone', 'live') }
    assert_equal 1, connections.status.dig('phone', 'packets')
    frames, = inbox.drain
    assert_in_delta 0.4, frames['phone']['jawOpen']
    # A failed rebind must restore the previous listener and leave the API alive.
    busy = UDPSocket.new
    busy.bind('127.0.0.1', 0)
    assert_raises(SystemCallError) { network.configure_tracking('127.0.0.1', [busy.addr[1]]) }
    receiver.send('jawOpen-20', 0, '127.0.0.1', port)
    Timeout.timeout(3) { sleep 0.01 until connections.status.dig('phone', 'packets') == 2 }
  ensure
    connections&.close
    network&.stop
    receiver&.close
    busy&.close
  end

  def test_invalid_settings_fail_before_connection
    connections = Live2D::Connections.new(nil, {})
    assert_raises(Live2D::Error) { connections.configure('listen_port' => 0) }
    assert_raises(Live2D::Error) { connections.configure('listen_bind' => 'invalid') }
    assert_raises(Live2D::Error) { connections.configure('webcam_backend' => 'unknown') }
  end

  def test_combined_guide_is_inside_every_platform
    margins = Live2D::Guides.state(Live2D::Guides.validate({}))['margins']
    Live2D::Guides::PRESETS.each_value do |preset|
      4.times { |i| assert_operator margins[i], :>=, preset[i] }
    end
    assert_raises(Live2D::Error) { Live2D::Guides.validate('preset' => 'Custom', 'margins' => [-1, 0, 0, 0]) }
  end
end
