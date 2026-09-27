# frozen_string_literal: true
require 'rbconfig'
require_relative '../lib/live2d_studio'
model = ARGV.fetch(0)
seconds = Float(ARGV[1] || 15)
codec = ARGV[2] || 'vp9'
fps = Integer(ARGV[3] || 30)
directory = File.join(Live2D::ROOT, 'output', "live-benchmark-#{Time.now.strftime('%Y%m%d-%H%M%S')}")
FileUtils.mkdir_p(directory)
config = Live2D.config
config['api']['port'] = TCPServer.open('127.0.0.1', 0) { |s| s.addr[1] }
config['tracking']['udp_ports'] = []
File.write(File.join(directory, 'config.yml'), YAML.dump(config))
token = SecureRandom.hex(24)
log = File.open(File.join(directory, 'studio.log'), 'wb')
pid = Process.spawn({ 'L2D_API_TOKEN' => token }, RbConfig.ruby, File.join(Live2D::ROOT, 'bin/studio'),
  '--model', model, '--config', File.join(directory, 'config.yml'), out: log, err: log, chdir: Live2D::ROOT)
socket = nil
begin
  Timeout.timeout(90) do
    loop do
      socket = TCPSocket.new('127.0.0.1', config['api']['port']) rescue nil
      break if socket
      sleep 0.1
    end
  end
  latencies = []
  call = lambda do |command|
    start = Live2D.monotonic
    socket.puts(JSON.generate(command.merge('token' => token)))
    raise 'UI/API unresponsive for 5 seconds' unless IO.select([socket], nil, nil, 5)
    result = JSON.parse(socket.gets)
    latencies << Live2D.monotonic - start
    raise result['error'] unless result['ok']
    result['result']
  end
  sleep 2
  baseline = call.call('op' => 'status')
  sleep 3
  ready = call.call('op' => 'status')
  baseline_fps = (ready['render_frames'] - baseline['render_frames']) / (ready['time'] - baseline['time'])
  output = File.join(directory, "odette-alpha#{Live2D::Exporter::CODECS.fetch(codec)[0]}")
  call.call('op' => 'record_start', 'output' => output, 'codec' => codec, 'fps' => fps)
  first = call.call('op' => 'status')
  samples = []
  deadline = Live2D.monotonic + seconds
  while Live2D.monotonic < deadline
    sleep 0.5
    samples << call.call('op' => 'status').reject { |k, _| %w[parameters attachments connection_settings].include?(k) }
    raise samples.last['error'] if samples.last.dig('export', 'state') == 'failed'
  end
  last = samples.last
  call.call('op' => 'ui_snapshot', 'path' => File.join(directory, 'recording-ui.png'))
  stop = Live2D.monotonic
  call.call('op' => 'record_stop')
  stop_latency = Live2D.monotonic - stop
  saved = nil
  Timeout.timeout(300) do
    loop do
      saved = call.call('op' => 'status')
      raise saved['error'] if saved.dig('export', 'state') == 'failed'
      break if saved.dig('export', 'state') == 'saved'
      sleep 0.5
    end
  end
  report = { baseline_fps: baseline_fps,
    recording_preview_fps: (last['render_frames'] - first['render_frames']) / (last['time'] - first['time']),
    max_api_latency: latencies.max, stop_latency: stop_latency, save_seconds: Live2D.monotonic - stop,
    final_export: saved['export'], samples: samples }
  File.write(File.join(directory, 'results.json'), JSON.pretty_generate(report))
  call.call('op' => 'ui_tab', 'tab' => 'Inputs')
  sleep 0.2
  call.call('op' => 'ui_snapshot', 'path' => File.join(directory, 'inputs-ui.png'))
  sleep 0.2
  call.call('op' => 'ui_tab', 'tab' => 'Guides')
  call.call('op' => 'guides', 'preset' => 'TikTok', 'persist' => false)
  sleep 0.2
  call.call('op' => 'ui_snapshot', 'path' => File.join(directory, 'guides-ui.png'))
  sleep 0.2
  puts JSON.pretty_generate(report.reject { |k, _| k == :samples })
  puts "Artifacts: #{directory}"
  call.call('op' => 'quit')
  Timeout.timeout(10) { Process.wait(pid) }
  pid = nil
ensure
  socket&.close
  if pid
    Process.kill('KILL', pid) rescue nil
    Process.wait(pid) rescue nil
  end
  log.close
end
