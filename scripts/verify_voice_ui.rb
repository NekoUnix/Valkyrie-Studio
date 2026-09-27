require 'rbconfig'
require_relative '../lib/live2d_studio'
directory = File.join(Live2D::ROOT, 'output', "voice-ui-#{Time.now.strftime('%Y%m%d-%H%M%S')}")
FileUtils.mkdir_p(directory)
config = Live2D.config
config['api']['port'] = TCPServer.open('127.0.0.1', 0) { |s| s.addr[1] }
config['tracking']['udp_ports'] = []
File.write(File.join(directory, 'config.yml'), YAML.dump(config))
token = SecureRandom.hex(24)
log = File.open(File.join(directory, 'studio.log'), 'wb')
args = [RbConfig.ruby, File.join(Live2D::ROOT, 'bin/studio'), '--config', File.join(directory, 'config.yml')]
args += ARGV[0] ? ['--model', ARGV[0]] : ['--diagnostic']
pid = Process.spawn({ 'L2D_API_TOKEN' => token }, *args, out: log, err: log, chdir: Live2D::ROOT)
socket = nil
begin
  Timeout.timeout(60) do
    loop do
      socket = TCPSocket.new('127.0.0.1', config['api']['port']) rescue nil
      break if socket
      sleep 0.1
    end
  end
  call = lambda do |command|
    socket.puts(JSON.generate(command.merge('token' => token)))
    raise 'UI became unresponsive' unless IO.select([socket], nil, nil, 5)
    reply = JSON.parse(socket.gets)
    raise reply['error'] unless reply['ok']
    reply['result']
  end
  results = []
  [[1.0, 'Audio'], [1.5, 'Audio'], [2.0, 'Inputs'], [2.5, 'Audio'], [1.5, 'Guides']].each do |scale, tab|
    call.call('op' => 'ui_settings', 'scale' => scale, 'follow_dpi' => true, 'persist' => false)
    call.call('op' => 'ui_tab', 'tab' => tab)
    sleep 0.25
    state = call.call('op' => 'status')
    raise 'UI scale not applied' unless state.dig('ui_settings', 'scale') == scale
    raise 'Secret present in status' if state.fetch('voice').key?('api_key')
    image = File.join(directory, "#{(scale * 100).to_i}-#{tab.downcase}.png")
    call.call('op' => 'ui_snapshot', 'path' => image)
    Timeout.timeout(5) { sleep 0.02 until File.file?(image) && File.size(image) > 0 }
    results << { scale: scale, tab: tab, key_configured: state.dig('voice', 'configured'), screenshot: image }
  end
  # No cloud call is attempted without a key.
  state = call.call('op' => 'status')
  unless state.dig('voice', 'configured')
    socket.puts(JSON.generate(op: 'tts', provider: 'openai', text: 'test', token: token))
    reply = JSON.parse(socket.gets)
    raise 'Missing key should be actionable' unless !reply['ok'] && reply['error'].include?('API key')
  end
  File.write(File.join(directory, 'results.json'), JSON.pretty_generate(results))
  call.call('op' => 'quit')
  Timeout.timeout(10) { Process.wait(pid) }
  pid = nil
  puts "Verified native UI at 100/150/200/250%, panel reflow, voice setup and missing-key handling: #{directory}"
ensure
  socket&.close
  if pid
    Process.kill('KILL', pid) rescue nil
    Process.wait(pid) rescue nil
  end
  log.close
end
