# frozen_string_literal: true
require 'json'
require 'yaml'
require 'socket'
require 'thread'
require 'fileutils'
require 'open3'
require 'securerandom'
require 'timeout'

module Live2D
  ROOT = File.expand_path('..', __dir__)
  class Error < StandardError; end

  def self.number(value, min: -1e6, max: 1e6)
    n = Float(value)
    raise Error, 'Number must be finite and in range' unless n.finite? && n.between?(min, max)
    n
  rescue ArgumentError, TypeError
    raise Error, 'Expected a number'
  end

  def self.monotonic
    Process.clock_gettime(Process::CLOCK_MONOTONIC)
  end

  def self.config(path = File.join(ROOT, 'config.yml'))
    YAML.safe_load_file(path)
  end
end

require_relative 'live2d_studio/tracking'
require_relative 'live2d_studio/engine'
require_relative 'live2d_studio/network'
require_relative 'live2d_studio/audio'
require_relative 'live2d_studio/voice'
require_relative 'live2d_studio/export'
require_relative 'live2d_studio/connections'
require_relative 'live2d_studio/guides'
