# frozen_string_literal: true
require_relative '../lib/live2d_studio'
input, output = ARGV
abort 'Usage: ruby scripts/rhubarb.rb audio.wav output.json' unless input && output
abort 'Output exists; choose a new path' if File.exist?(output)
clip = Live2D::Audio::Clip.new(input)
begin
  _, stderr, status = Open3.capture3(ENV.fetch('RHUBARB', 'rhubarb'), '-f', 'json', '-o', File.expand_path(output), clip.pcm_path)
  raise Live2D::Error, "Rhubarb failed: #{stderr}" unless status.success?
  puts "Visemes saved: #{File.expand_path(output)}"
ensure
  clip.close
end
