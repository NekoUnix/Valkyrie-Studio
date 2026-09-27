# frozen_string_literal: true
require 'rbconfig'
require_relative '../lib/live2d_studio'
model_path = ARGV.fetch(0) { abort 'Usage: ruby scripts/integration_native.rb MODEL_PATH' }
output = File.join(Live2D::ROOT, 'output', "integration-#{Time.now.strftime('%Y%m%d-%H%M%S')}")
FileUtils.mkdir_p(output)
token = SecureRandom.hex(24)
test_config = Live2D.config
test_config['api']['port'] = TCPServer.open('127.0.0.1', 0) { |s| s.addr[1] }
test_config['tracking']['udp_ports'] = []
config_path = File.join(output, 'config.yml')
File.write(config_path, YAML.dump(test_config))
log = File.open(File.join(output, 'studio.log'), 'wb')
pid = Process.spawn({ 'L2D_API_TOKEN' => token }, RbConfig.ruby, File.join(Live2D::ROOT, 'bin', 'studio'),
  '--hidden', '--model', model_path, '--width', '320', '--height', '568', '--config', config_path,
  out: log, err: log, chdir: Live2D::ROOT)
socket = nil
results = []
begin
  Timeout.timeout(60) do
    loop do
      begin
        socket = TCPSocket.new('127.0.0.1', test_config['api']['port'])
        break
      rescue Errno::ECONNREFUSED
        sleep 0.1
      end
    end
  end
  call = lambda do |command|
    socket.write(JSON.generate(command.merge('token' => token)) + "\n")
    raise 'API timeout' unless IO.select([socket], nil, nil, 45)
    result = JSON.parse(socket.gets)
    raise "#{command['op']}: #{result['error']}" unless result['ok']
    results << { 'op' => command['op'], 'ok' => true }
    result['result']
  end
  schema = call.call('op' => 'schema')
  raise 'No parameters' if schema['parameters'].empty?
  File.write(File.join(output, 'model-info.json'), JSON.pretty_generate(schema))
  call.call('op' => 'emotion', 'name' => 'joy', 'duration' => 3)
  call.call('op' => 'parameters', 'values' => { 'ParamAngleX' => 12 }, 'duration' => 2)
  call.call('op' => 'snapshot', 'path' => File.join(output, 'portrait.png'))
  [[1920, 1080], [1080, 1080], [1080, 1920]].each do |w, h|
    call.call('op' => 'canvas', 'width' => w, 'height' => h)
    call.call('op' => 'snapshot', 'path' => File.join(output, "canvas-#{w}x#{h}.png"))
  end
  call.call('op' => 'canvas', 'width' => 320, 'height' => 568)
  Live2D::Guides::PRESETS.each_key do |preset|
    call.call('op' => 'guides', 'preset' => preset, 'persist' => false)
    raise 'Guide selection was not applied' unless call.call('op' => 'status').dig('guides', 'preset') == preset
  end
  call.call('op' => 'guides', 'preset' => 'All platforms', 'persist' => false)
  receiver = UDPSocket.new
  receiver.bind('127.0.0.1', 0)
  listen = UDPSocket.new
  listen.bind('127.0.0.1', 0)
  listen_port = listen.addr[1]
  listen.close
  call.call('op' => 'connection_start', 'source' => 'phone', 'persist' => false, 'settings' => {
    'phone_protocol' => 'ifacial', 'phone_ip' => '127.0.0.1', 'phone_port' => receiver.addr[1],
    'listen_bind' => '127.0.0.1', 'listen_port' => listen_port, 'webcam_port' => listen_port })
  raise 'Phone startup request missing' unless IO.select([receiver], nil, nil, 2)
  raise 'Wrong phone request' unless receiver.recv(1024).start_with?('iFacialMocap_')
  receiver.send('jawOpen-40|eyeBlink_L-10', 0, '127.0.0.1', listen_port)
  Timeout.timeout(3) { sleep 0.02 until call.call('op' => 'status').dig('connections', 'phone', 'live') }
  call.call('op' => 'connection_stop', 'source' => 'phone')
  receiver.close
  call.call('op' => 'prop', 'path' => File.join(output, 'portrait.png'), 'x' => 0.4, 'y' => 0.1)
  call.call('op' => 'attachment', 'index' => 0, 'scale' => 0.25, 'rotation' => 0.5)
  call.call('op' => 'snapshot', 'path' => File.join(output, 'prop-anchor.png'))
  call.call('op' => 'attachment_remove', 'index' => 0)
  call.call('op' => 'load_model', 'path' => model_path, 'primary' => false)
  call.call('op' => 'snapshot', 'path' => File.join(output, 'accessory.png'))
  call.call('op' => 'attachment_remove', 'index' => 0)
  call.call('op' => 'background', 'color' => [0.04, 0.1, 0.12, 1])
  ffmpeg = ENV.fetch('FFMPEG', 'ffmpeg')
  audio = File.join(output, 'tone.wav')
  _, error, code = Open3.capture3(ffmpeg, '-v', 'error', '-f', 'lavfi', '-i', 'sine=frequency=440:duration=0.8', audio)
  raise error unless code.success?
  call.call('op' => 'audio', 'path' => audio)
  Timeout.timeout(15) do
    loop do
      break if call.call('op' => 'status')['audio']
      sleep 0.1
    end
  end
  background = File.join(output, 'background.mp4')
  _, error, code = Open3.capture3(ffmpeg, '-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=160x90:rate=30:duration=0.2', '-c:v', 'libx264', background)
  raise error unless code.success?
  # A generated video file exercises helper startup/inference/error reporting
  # without opening the user's physical camera or contacting an IP camera.
  call.call('op' => 'connection_start', 'source' => 'webcam', 'persist' => false,
    'settings' => { 'webcam_source' => background, 'webcam_backend' => 'auto' })
  Timeout.timeout(30) do
    loop do
      message = call.call('op' => 'status').dig('connections', 'webcam', 'message')
      break if message.start_with?('Helper exited')
      sleep 0.1
    end
  end
  call.call('op' => 'connection_stop', 'source' => 'webcam')
  helper_log = File.read(File.join(Live2D::ROOT, 'tmp/webcam-connection.log'))
  raise 'Webcam helper did not process the synthetic input' unless helper_log.match?(/after [1-9]\d* frames/)
  call.call('op' => 'background', 'path' => background)
  video = File.join(output, 'recording.mp4')
  call.call('op' => 'record_start', 'output' => video, 'codec' => 'h264')
  sleep 0.5
  call.call('op' => 'record_stop')
  Timeout.timeout(60) do
    loop do
      result = call.call('op' => 'status')
      raise result['error'] if result.dig('export', 'state') == 'failed'
      break if result.dig('export', 'state') == 'saved'
      sleep 0.1
    end
  end
  call.call('op' => 'audio_stop')
  raise 'Recording empty' unless File.size(video) > 0
  call.call('op' => 'quit')
  Timeout.timeout(10) { Process.wait(pid) }
  pid = nil
  File.write(File.join(output, 'results.json'), JSON.pretty_generate(results))
  puts "Native integration: #{results.size} successful operations. Results: #{output}"
ensure
  socket&.close
  if pid
    Process.kill('KILL', pid) rescue nil
    Process.wait(pid) rescue nil
  end
  log.close
end
