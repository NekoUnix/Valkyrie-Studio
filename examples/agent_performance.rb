#!/usr/bin/env ruby
# frozen_string_literal: true
# A real-time example agent. Edit the motion/expression schedule to make your own.
require 'json'
require 'socket'
require 'securerandom'
require_relative '../lib/live2d_studio'

port = Integer(ENV.fetch('L2D_API_PORT', '4141'))
token_file = File.join(Live2D::ROOT, 'tmp', port == 4141 ? 'api-token' : "api-token-#{port}")
token = ENV['L2D_API_TOKEN'] || File.read(token_file).strip
seconds = Float(ARGV.fetch(0, '14'))
socket = TCPSocket.new('127.0.0.1', port)
send_command = lambda do |command|
  socket.write(JSON.generate(command.merge('token' => token, 'id' => SecureRandom.hex(4))) + "\n")
  raise 'Studio response timed out' unless IO.select([socket], nil, nil, 10)
  reply = JSON.parse(socket.gets || raise('Studio disconnected'))
  raise reply.fetch('error', 'Agent command failed') unless reply['ok']
  reply['result']
end

begin
  send_command.call('op' => 'mode', 'mode' => 'agent')
  status = send_command.call('op' => 'status')
  raise 'Load a model in Scene before running the agent' unless status.dig('agent', 'ready')
  puts "Agent connected to #{status['model']}"
  send_command.call('op' => 'emotion', 'name' => 'joy', 'duration' => 3)
  start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  tick = 0
  loop do
    elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - start
    break if elapsed >= seconds
    send_command.call('op' => 'emotion', 'name' => 'thinking', 'duration' => 3) if tick == 4 * 24
    send_command.call('op' => 'emotion', 'name' => 'surprised', 'duration' => 2) if tick == 8 * 24
    phase = elapsed * 1.5
    blink = (elapsed % 3.2) > 3.05 ? 1.0 : 0.0
    send_command.call('op' => 'tracking', 'values' => {
      'yaw' => Math.sin(phase) * 21, 'pitch' => Math.sin(phase * 0.65) * 8,
      'roll' => Math.sin(phase * 0.7) * 5,
      'eyeBlinkLeft' => blink, 'eyeBlinkRight' => blink,
      'eyeLookOutLeft' => [Math.sin(phase) * 0.3, 0].max,
      'eyeLookOutRight' => [Math.sin(phase) * 0.3, 0].max
    })
    tick += 1
    sleep_time = start + tick / 24.0 - Process.clock_gettime(Process::CLOCK_MONOTONIC)
    sleep(sleep_time) if sleep_time.positive?
  end
  send_command.call('op' => 'tracking', 'values' => { 'yaw' => 0, 'pitch' => 0, 'roll' => 0 })
  puts "Finished: #{send_command.call('op' => 'status').dig('agent', 'commands')} control commands"
ensure
  socket.close
end
