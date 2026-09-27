# frozen_string_literal: true
require 'ffi'
module Live2D
  module Native
    extend FFI::Library
    paths = [ENV['L2D_BRIDGE'], *Dir.glob('build/bin/**/*live2d_native_bridge.{dll,so,dylib}', base: ROOT).map { |p| File.join(ROOT, p) }].compact.reject(&:empty?)
    raise Error, 'Native bridge missing. Run scripts/compile.sh or scripts/compile.bat.' if paths.empty?
    # RubyInstaller restricts DLL dependency search. Preload only our adjacent,
    # explicitly resolved binaries instead of changing global process DLL paths.
    if FFI::Platform.windows?
      DEPENDENCIES = %w[glfw3.dll PurismCore.dll].filter_map do |name|
        file = File.join(File.dirname(paths.first), name)
        FFI::DynamicLibrary.open(file, FFI::DynamicLibrary::RTLD_LAZY) if File.file?(file)
      end
    end
    ffi_lib paths.first
    {
      l2d_last_error: [[], :string], l2d_init: [[], :int], l2d_shutdown: [[], :void],
      l2d_load_model: [[:string], :pointer], l2d_destroy_model: [[:pointer], :void],
      l2d_model_info: [[:pointer], :string], l2d_get_param_count: [[:pointer], :int],
      l2d_get_param_ids: [[:pointer], :string], l2d_set_parameter: [[:pointer, :string, :float], :int],
      l2d_set_parameters: [[:pointer, :pointer, :int], :int], l2d_update: [[:pointer, :float], :int],
      l2d_draw: [[:pointer, :uint, :pointer], :int], l2d_anchor: [[:pointer, :int, :pointer], :int],
      l2d_nearest: [[:pointer, :float, :float], :int], l2d_canvas_create: [[:int, :int], :uint],
      l2d_canvas_destroy: [[:uint], :void], l2d_canvas_texture: [[:uint], :uint],
      l2d_canvas_clear: [[:uint, :float, :float, :float, :float], :int],
      l2d_read_rgba: [[:uint, :pointer, :size_t], :int], l2d_texture_load: [[:string], :uint],
      l2d_capture_submit: [[:uint, :int64], :int], l2d_capture_poll: [[:pointer, :size_t, :pointer], :int],
      l2d_capture_reset: [[], :void],
      l2d_texture_rgba: [[:uint, :int, :int, :pointer], :uint], l2d_texture_destroy: [[:uint], :void],
      l2d_texture_size: [[:uint, :pointer], :int],
      l2d_draw_texture: [[:uint, :uint, :float, :float, :float, :float, :float], :int],
      l2d_diagnostic: [[:uint, :float], :int], l2d_ui_init: [[:pointer], :int], l2d_ui_shutdown: [[], :void],
      l2d_ui_frame: [[:uint, :int, :int, :string], :string], l2d_ui_canvas_point: [[:double, :double, :pointer], :int],
      l2d_ui_snapshot: [[:string], :int],
      l2d_audio_init: [[], :int], l2d_audio_load: [[:string], :int], l2d_audio_play: [[], :int],
      l2d_audio_stop: [[], :void], l2d_audio_time: [[], :double], l2d_audio_shutdown: [[], :void]
    }.each do |name, (args, result)|
      blocking = %i[l2d_load_model l2d_update l2d_draw l2d_read_rgba l2d_capture_poll l2d_capture_submit l2d_texture_load
        l2d_audio_init l2d_audio_load l2d_audio_shutdown].include?(name)
      attach_function name, args, result, blocking: blocking
    end
    def self.check(value)
      raise Error, l2d_last_error.to_s if value == 0 || value.nil? || (value.respond_to?(:null?) && value.null?)
      value
    end
  end

  module GLFW
    extend FFI::Library
    ffi_lib [ENV['GLFW_LIBRARY'], *Dir.glob('build/bin/**/*glfw*.{dll,so,dylib}', base: ROOT).map { |p| File.join(ROOT, p) }, 'glfw3', 'glfw'].compact.reject(&:empty?)
    callback :drop, [:pointer, :int, :pointer], :void
    attach_function :glfwInit, [], :int
    attach_function :glfwTerminate, [], :void
    attach_function :glfwWindowHint, [:int, :int], :void
    attach_function :glfwCreateWindow, [:int, :int, :string, :pointer, :pointer], :pointer
    attach_function :glfwDestroyWindow, [:pointer], :void
    attach_function :glfwMakeContextCurrent, [:pointer], :void
    attach_function :glfwSwapInterval, [:int], :void
    attach_function :glfwSwapBuffers, [:pointer], :void
    attach_function :glfwPollEvents, [], :void
    attach_function :glfwWindowShouldClose, [:pointer], :int
    attach_function :glfwSetWindowShouldClose, [:pointer, :int], :void
    attach_function :glfwSetDropCallback, [:pointer, :drop], :pointer
    attach_function :glfwGetCursorPos, [:pointer, :pointer, :pointer], :void
    attach_function :glfwSetWindowTitle, [:pointer, :string], :void
  end

  class Model
    attr_reader :handle, :info, :mapper, :path
    def initialize(path)
      @path = File.expand_path(path)
      @handle = Native.check(Native.l2d_load_model(@path))
      @info = JSON.parse(Native.l2d_model_info(@handle))
      @mapper = Tracking::Mapper.new(@info.fetch('parameters'))
      @ids = @info.fetch('parameters').map { |p| p.fetch('id') }
      @parameter_buffer = FFI::MemoryPointer.new(:float, @ids.length)
      @matrix_buffer = FFI::MemoryPointer.new(:float, 5)
      @anchor_buffer = FFI::MemoryPointer.new(:float, 4)
    end
    def update(parameters, dt)
      @parameter_buffer.write_array_of_float(@ids.map { |id| parameters.fetch(id) })
      Native.check(Native.l2d_set_parameters(@handle, @parameter_buffer, @ids.length))
      Native.check(Native.l2d_update(@handle, dt))
    end
    def draw(fbo, transform)
      @matrix_buffer.write_array_of_float(transform.size == 4 ? transform + [0] : transform)
      Native.check(Native.l2d_draw(@handle, fbo, @matrix_buffer))
    end
    def anchor(index)
      Native.check(Native.l2d_anchor(@handle, index, @anchor_buffer))
      @anchor_buffer.read_array_of_float(4)
    end
    def nearest(x, y) = Native.l2d_nearest(@handle, x, y)
    def close
      Native.l2d_destroy_model(@handle) if @handle
      @handle = nil
    end
  end

  class Renderer
    attr_reader :width, :height, :fbo
    def initialize(width, height) = resize(width, height)
    def resize(width, height)
      raise Error, 'Canvas dimensions must be even integers between 16 and 8192' unless [width, height].all? { |n| n.is_a?(Integer) && n.even? && n.between?(16, 8192) }
      new_fbo = Native.check(Native.l2d_canvas_create(width, height))
      Native.l2d_canvas_destroy(@fbo) if @fbo
      @fbo, @width, @height = new_fbo, width, height
      @pixels = FFI::MemoryPointer.new(:uchar, width * height * 4)
      @capture_index = FFI::MemoryPointer.new(:int64)
    end
    def clear(color) = Native.check(Native.l2d_canvas_clear(@fbo, *color))
    def texture = Native.l2d_canvas_texture(@fbo)
    def rgba
      Native.check(Native.l2d_read_rgba(@fbo, @pixels, @pixels.size))
      @pixels.read_bytes(@pixels.size)
    end
    def capture_submit(frame)
      result = Native.l2d_capture_submit(@fbo, frame)
      raise Error, Native.l2d_last_error if result < 0
      result == 1
    end
    def capture_poll
      result = Native.l2d_capture_poll(@pixels, @pixels.size, @capture_index)
      raise Error, Native.l2d_last_error if result < 0
      [@pixels.read_bytes(@pixels.size), @capture_index.read_int64] if result == 1
    end
    def close = Native.l2d_canvas_destroy(@fbo)
  end
end
