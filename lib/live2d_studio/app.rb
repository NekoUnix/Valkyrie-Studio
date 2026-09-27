# frozen_string_literal: true
require_relative 'native'
module Live2D
  class App
    def initialize(config, options)
      @config, @options = config, options
      @inbox = Inbox.new
      @engine = Engine.new(config.fetch('tracking'))
      @attachments, @color, @view = [], [0, 0, 0, 0], [1.0, 0.0, 0.0]
      @fps = Live2D.number(options.fetch(:fps, config.dig('canvas', 'fps')), min: 1, max: 120)
      @time, @frames, @notice, @error = 0.0, 0, 'Drop your model to begin.', ''
      @jobs = Queue.new
      @agent_commands = 0
      @agent_last_control = nil
      @settings_path = File.join(ROOT, 'user-settings.json')
      @preferences = File.file?(@settings_path) ? JSON.parse(File.read(@settings_path)) : {}
      @guides = Guides.validate(@preferences.fetch('guides', {}))
      @ui_settings = { 'scale' => 1.0, 'follow_dpi' => true }.merge(@preferences.fetch('ui', {}))
      @voice = Audio::Voice.new(@preferences.fetch('voice', {}))
      @elevenlabs = Audio::ElevenLabs.new(@preferences.fetch('elevenlabs', {}))
      @timeline = Timeline.new(options[:timeline]) if options[:timeline]
      @frame_limit = options[:frames] || (@timeline && (@timeline.duration * @fps).ceil)
      @offline = !!@frame_limit
      @record_fps = @offline ? @fps : options.fetch(:fps, config.dig('export', 'fps') || 30)
    end

    def run
      raise Error, 'GLFW could not initialize a native display' if GLFW.glfwInit.zero?
      GLFW.glfwWindowHint(0x00022002, 3) # CONTEXT_VERSION_MAJOR
      GLFW.glfwWindowHint(0x00022003, 3)
      GLFW.glfwWindowHint(0x00022008, 0x00032001) # OPENGL_CORE_PROFILE
      GLFW.glfwWindowHint(0x00022006, 1) # FORWARD_COMPAT, required on macOS
      GLFW.glfwWindowHint(0x00020004, @options[:hidden] ? 0 : 1)
      @window = GLFW.glfwCreateWindow(@config.dig('window', 'width'), @config.dig('window', 'height'), 'Valkyrie Studio', nil, nil)
      raise Error, 'OpenGL 3.3 Core window creation failed' if @window.null?
      GLFW.glfwMakeContextCurrent(@window)
      GLFW.glfwSwapInterval(@offline ? 0 : 1)
      Native.check(Native.l2d_init)
      @renderer = Renderer.new(@options.fetch(:width, @config.dig('canvas', 'width')), @options.fetch(:height, @config.dig('canvas', 'height')))
      Native.check(Native.l2d_ui_init(@window)) unless @options[:hidden]
      @audio_available = Native.l2d_audio_init != 0 unless @offline
      @notice = 'Audio device unavailable; offline audio/export still works.' if @audio_available == false
      install_drop
      load_model(@options[:model], true) if @options[:model]
      load_clip(Audio::Clip.new(@options[:audio], cues_path: @options[:visemes])) if @options[:audio]
      if @options[:background]
        command('op' => 'background', 'path' => @options[:background])
      end
      @network = Network.new(@config, @inbox).start unless @offline
      if @network
        @connections = Connections.new(@network, @preferences.fetch('connections', {}))
        FileUtils.mkdir_p(File.join(ROOT, 'tmp'))
        token_name = @config.dig('api', 'port') == 4141 ? 'api-token' : "api-token-#{@config.dig('api', 'port')}"
        token_file = File.join(ROOT, 'tmp', token_name)
        File.write(token_file, @network.token, mode: 'w', perm: 0o600)
        puts "Native studio API: #{@config.dig('api', 'bind')}:#{@config.dig('api', 'port')} (token: #{token_file})"
      end
      @timeline&.due(0)&.each { |c| command(c) }
      record_start(@options[:output], @options.fetch(:codec, 'vp9')) if @options[:output]
      @start = @last_wall = Live2D.monotonic
      refresh_elevenlabs_voices if @elevenlabs.configured? && !@offline
      loop do
        if GLFW.glfwWindowShouldClose(@window) != 0 || (@quit_at && Live2D.monotonic >= @quit_at)
          record_stop if @exporter
          @closing = true
          GLFW.glfwSetWindowShouldClose(@window, 0)
        end
        break if (@closing && !@finalizer&.alive?) || (@frame_limit && @frames >= @frame_limit)
        now = Live2D.monotonic
        wall_dt = (now - @last_wall).clamp(0.0001, 10)
        dt = @offline ? 1.0 / @fps : wall_dt.clamp(0.0001, 0.1)
        @last_wall = now
        @time = @offline ? @frames.to_f / @fps : now - @start
        @timeline&.due(@time)&.each { |c| command(c) }
        process_inputs unless @offline
        process_jobs
        render(dt)
        capture
        if @options[:snapshot] && (@frame_limit.nil? || @frames == @frame_limit - 1)
          snapshot(@options.delete(:snapshot))
        end
        ui(wall_dt) unless @options[:hidden]
        GLFW.glfwPollEvents
        @frames += 1
      end
      record_stop if @exporter
      @finalizer&.join
      process_jobs
      File.write(@options[:report], JSON.pretty_generate(status.merge('model_info' => @model&.info, 'frames' => @frames))) if @options[:report]
    ensure
      close
    end

    def process_inputs
      frames, commands = @inbox.drain
      frames.each { |source, values| @engine.ingest(source, values, @time) }
      commands.each do |request, reply|
        begin
          response = command(request)
          if reply && %w[tracking emotion parameters parameters_clear].include?(request['op'])
            @agent_commands += 1
            @agent_last_control = Live2D.monotonic
          end
          reply << { ok: true, result: response } if reply
        rescue StandardError => e
          @error = e.message
          reply << { ok: false, error: e.message } if reply
        end
      end
      @error = @network.errors.pop unless @network.nil? || @network.errors.empty?
    end

    def command(c)
      raise Error, 'Command must be a JSON object' unless c.is_a?(Hash)
      case c.fetch('op')
      when 'ui_settings'
        @ui_settings['scale'] = Live2D.number(c.fetch('scale', @ui_settings['scale']), min: 0.75, max: 2.5)
        @ui_settings['follow_dpi'] = !!c.fetch('follow_dpi', @ui_settings['follow_dpi'])
        save_preferences unless c['persist'] == false
      when 'voice_configure'
        raise Error, 'Wait for speech generation to finish before changing voice settings' if @job_thread&.alive?
        @voice.configure(c.fetch('settings', {}))
        @voice.set_key(c['api_key'], remember: c.fetch('remember', true)) if c['api_key'] && !c['api_key'].empty?
        save_preferences unless c['persist'] == false
        @error = ''
        @notice = @voice.message
      when 'voice_forget_key'
        raise Error, 'Wait for speech generation to finish first' if @job_thread&.alive?
        @voice.forget_key
        @notice = @voice.message
      when 'elevenlabs_configure'
        raise Error, 'Wait for the current audio job to finish' if @job_thread&.alive?
        @elevenlabs.configure(c.fetch('settings', {}))
        if c['api_key'] && !c['api_key'].empty?
          @elevenlabs.set_key(c['api_key'], remember: c.fetch('remember', true))
          refresh_elevenlabs_voices
        end
        save_preferences unless c['persist'] == false
        @error = ''
        @notice = @elevenlabs.message
      when 'elevenlabs_refresh_voices'
        refresh_elevenlabs_voices
      when 'elevenlabs_forget_key'
        raise Error, 'Wait for the current audio job to finish' if @job_thread&.alive?
        @elevenlabs.forget_key
        @notice = @elevenlabs.message
      when 'guides'
        @guides = Guides.validate(@guides.merge(c.reject { |k, _| k == 'op' }))
        save_preferences unless c['persist'] == false
      when 'connection_start'
        raise Error, 'Connections are unavailable during offline rendering' unless @connections
        @connections.configure(c.fetch('settings', {}))
        @connections.start(c.fetch('source'))
        @engine.mode = c.fetch('source')
        @engine.calibration.reset
        save_preferences unless c['persist'] == false
      when 'connection_stop'
        @connections&.stop(c.fetch('source'))
        @engine.mode = 'idle' if @engine.mode == c.fetch('source')
      when 'status' then return status
      when 'schema' then return @model ? @model.info : { 'parameters' => [] }
      when 'mode' then @engine.mode = c.fetch('mode')
      when 'tracking' then @engine.ingest(c.fetch('source', 'agent'), c.fetch('values'), @time)
      when 'calibrate' then @engine.calibration.reset
      when 'parameters_clear' then @engine.clear_parameters
      when 'emotion'
        @engine.emotion(c.fetch('name'), c.fetch('intensity', 1), @time + Live2D.number(c.fetch('duration', 3), min: 0, max: 600))
      when 'parameters'
        raise Error, 'Load a model before setting parameters' unless @model
        values = c.fetch('values')
        raise Error, 'values must be an object' unless values.is_a?(Hash)
        unknown = values.keys - @model.mapper.schema.keys
        raise Error, "Unknown parameters: #{unknown.join(', ')}" unless unknown.empty?
        @engine.parameters(values, @time + Live2D.number(c.fetch('duration', 1), min: 0, max: 600))
      when 'canvas'
        raise Error, 'Finish recording before resizing the canvas' if @exporter
        @renderer.resize(c.fetch('width'), c.fetch('height'))
        set_background(@background_path) if @video
      when 'view'
        @view = [Live2D.number(c.fetch('zoom', 1), min: 0.1, max: 30), Live2D.number(c.fetch('x', 0), min: -12, max: 12), Live2D.number(c.fetch('y', 0), min: -12, max: 12)]
      when 'load_model' then load_model(c.fetch('path'), c.fetch('primary', @model.nil?), c.fetch('x', 0), c.fetch('y', 0))
      when 'prop' then add_prop(c.fetch('path'), c.fetch('x', 0), c.fetch('y', 0))
      when 'attachment'
        a = @attachments.fetch(Integer(c.fetch('index')))
        a[:scale] = Live2D.number(c.fetch('scale', a[:scale]), min: 0.01, max: 10)
        a[:rotation] = Live2D.number(c.fetch('rotation', a[:rotation]), min: -Math::PI * 2, max: Math::PI * 2)
        anchor = Integer(c.fetch('anchor', a[:anchor]))
        raise Error, 'Invalid ArtMesh index' unless anchor.between?(-1, @model.info['drawables'].size - 1)
        if a[:anchor] != anchor
          a[:anchor], a[:baseline] = anchor, anchor < 0 ? nil : @model.anchor(anchor)
        end
      when 'attachment_remove'
        index = Integer(c.fetch('index'))
        raise Error, 'Invalid attachment index' unless index.between?(0, @attachments.length - 1)
        dispose_attachment(@attachments.delete_at(index))
      when 'background'
        if c['path']
          set_background(c['path'])
        else
          color = c.fetch('color')
          raise Error, 'color must contain RGBA' unless color.is_a?(Array) && color.size == 4
          color = color.map { |v| Live2D.number(v, min: 0, max: 1) }
          clear_background
          @color = color
        end
      when 'audio'
        raise Error, 'Finish recording and saving before replacing audio' if @exporter || @finalizer&.alive?
        if @offline
          load_clip(Audio::Clip.new(c.fetch('path'), cues_path: c['visemes']))
        else
          job { Audio::Clip.new(c.fetch('path'), cues_path: c['visemes']) }
        end
      when 'audio_play'
        raise Error, 'Load audio first' unless @clip
        @audio_started = @time
        if @audio_available
          Native.l2d_audio_stop
          raise Error, 'Audio playback failed' if Native.l2d_audio_play.zero?
        end
      when 'audio_stop'
        raise Error, 'Stop recording before stopping its audio' if @exporter
        Native.l2d_audio_stop
        @audio_started = nil
      when 'tts'
        raise Error, 'Generate speech before starting an offline render; pass the resulting audio with --audio' if @offline
        raise Error, 'Finish recording and saving before generating speech' if @exporter || @finalizer&.alive?
        provider = c.fetch('provider', 'openai')
        settings = @voice.settings.merge(c.slice('voice', 'model', 'speed', 'instructions'))
        @voice.validate(settings) if provider == 'openai'
        raise Error, 'Add your OpenAI API key under Voice setup first' if provider == 'openai' && !@voice.configured?
        raise Error, 'Add your ElevenLabs API key under Voice setup first' if provider == 'elevenlabs' && !@elevenlabs.configured?
        autoplay = c.fetch('autoplay', provider == 'openai' ? @voice.settings['autoplay'] :
          provider == 'elevenlabs' && @elevenlabs.settings['autoplay'])
        job(kind: autoplay ? :clip_play : :clip) do
          FileUtils.mkdir_p(File.join(ROOT, 'output'))
          ext = provider == 'elevenlabs' ? '.mp3' : '.wav'
          output = File.join(ROOT, 'output', "speech-#{SecureRandom.hex(5)}#{ext}")
          if provider == 'openai'
            @voice.synthesize(text: c.fetch('text'), output: output, settings: settings)
          elsif provider == 'elevenlabs'
            @elevenlabs.synthesize(text: c.fetch('text'), output: output, voice: c['voice'], model: c['model'])
          else
            Audio::TTS.new.synthesize(provider: provider, text: c.fetch('text'), output: output, voice: c['voice'])
          end
          Audio::Clip.new(output)
        end
      when 'record_start' then record_start(c.fetch('output'), c.fetch('codec', 'vp9'), c.fetch('fps', @record_fps))
      when 'record_stop' then return record_stop
      when 'snapshot' then snapshot(c.fetch('path'))
      when 'ui_snapshot' then @options[:ui_snapshot] = File.expand_path(c.fetch('path'))
      when 'ui_tab'
        tab = c.fetch('tab')
        raise Error, 'Unknown tab' unless %w[Scene Inputs Guides Audio].include?(tab)
        @ui_tab_request = { 'tab' => tab, 'id' => SecureRandom.hex(4) }
      when 'quit' then @quit_at = Live2D.monotonic + 0.15
      else raise Error, "Unknown operation: #{c['op']}"
      end
      true
    end

    def load_model(path, primary, x = 0, y = 0)
      raise Error, 'Finish recording before changing rigs' if @exporter
      candidate = Model.new(path)
      if primary || !@model
        @attachments.each { |a| dispose_attachment(a) }; @attachments.clear
        @model&.close
        @model = candidate
        @engine.clear_parameters
        @engine.mode = @engine.mode
      else
        add_attachment(model: candidate, path: path, x: x, y: y, scale: 0.3)
      end
      @notice = "Loaded #{candidate.info['parameters'].size} parameters and #{candidate.info['textures']} textures."
      @error = ''
    end

    def transform
      return [1, 1, 0, 0] unless @model
      w, h = @model.info.values_at('canvas_width', 'canvas_height')
      aspect = @renderer.width.to_f / @renderer.height
      sy = [1.8 / h, 1.8 * aspect / w].min * @view[0]
      [sy / aspect, sy, @view[1], @view[2]]
    end

    def add_prop(path, x, y)
      raise Error, 'Load a primary model to anchor a prop' unless @model
      id = Native.check(Native.l2d_texture_load(File.expand_path(path)))
      add_attachment(texture: id, path: path, x: x, y: y, scale: 0.2)
    end

    def add_attachment(model: nil, texture: nil, path:, x:, y:, scale:)
      x, y = Live2D.number(x, min: -4, max: 4), Live2D.number(y, min: -4, max: 4)
      sx, sy, tx, ty = transform
      mx, my = (x - tx) / sx, (y - ty) / sy
      anchor = @model.nearest(mx, my)
      aspect = 1.0
      if texture
        dimensions = FFI::MemoryPointer.new(:int, 2)
        Native.check(Native.l2d_texture_size(texture, dimensions))
        w, h = dimensions.read_array_of_int(2)
        aspect = w.to_f / h
      end
      @attachments << { model: model, texture: texture, name: File.basename(path), path: File.expand_path(path),
        x: mx, y: my, anchor: anchor, baseline: anchor < 0 ? nil : @model.anchor(anchor), scale: scale, rotation: 0.0, aspect: aspect }
    end

    def attachment_pose(a)
      x, y, angle, scale = a.values_at(:x, :y, :rotation, :scale)
      if a[:baseline] && a[:anchor] >= 0
        bx, by, ba, bl = a[:baseline]
        cx, cy, ca, cl = @model.anchor(a[:anchor])
        r = ca - ba
        ratio = bl > 1e-6 ? (cl / bl).clamp(0.05, 20) : 1
        dx, dy = x - bx, y - by
        x, y = cx + (dx * Math.cos(r) - dy * Math.sin(r)) * ratio, cy + (dx * Math.sin(r) + dy * Math.cos(r)) * ratio
        angle += r
        scale *= ratio
      end
      sx, sy, tx, ty = transform
      [x * sx + tx, y * sy + ty, angle, scale * @view[0]]
    end

    def render(dt)
      @renderer.clear(@color)
      if @video
        target = @offline ? @frames : (@time * @fps).floor
        while @video_frame < target
          @background_buffer.put_bytes(0, @video.frame)
          @background_texture = Native.check(Native.l2d_texture_rgba(@background_texture || 0, @renderer.width, @renderer.height, @background_buffer))
          @video_frame += 1
        end
      end
      if @background_texture
        dimensions = FFI::MemoryPointer.new(:int, 2)
        Native.check(Native.l2d_texture_size(@background_texture, dimensions))
        w, h = dimensions.read_array_of_int(2)
        ratio = (w.to_f / h) / (@renderer.width.to_f / @renderer.height)
        Native.check(Native.l2d_draw_texture(@background_texture, @renderer.fbo, 0, 0,
          ratio >= 1 ? 2 * ratio : 2, ratio >= 1 ? 2 : 2 / ratio, 0))
      end
      if @model
        audio_time = if @audio_started
          @audio_available && !@offline ? Native.l2d_audio_time : @time - @audio_started
        end
        mouth = @clip.mouth(audio_time) if audio_time
        @model_parameters = @engine.sample(@model.mapper, @time, dt, mouth: mouth)
        @model.update(@model_parameters, dt)
        @model.draw(@renderer.fbo, transform)
        @attachments.each_with_index do |a, i|
          x, y, rotation, scale = attachment_pose(a)
          if a[:model]
            rig = a[:model]
            rig.update(@engine.sample(rig.mapper, @time, dt, mouth: mouth, key: rig.object_id), dt)
            sy = scale * 2 / rig.info['canvas_height']
            rig.draw(@renderer.fbo, [sy * @renderer.height / @renderer.width, sy, x, y, rotation])
          else
            Native.check(Native.l2d_draw_texture(a[:texture], @renderer.fbo, x, y,
              scale * a[:aspect] * @renderer.height / @renderer.width, scale, rotation))
          end
        end
      elsif @options[:diagnostic]
        Native.check(Native.l2d_diagnostic(@renderer.fbo, @time))
      end
    end

    def set_background(path)
      raise Error, 'Background file missing' unless File.file?(path)
      clear_background
      @background_path = File.expand_path(path)
      if File.extname(path).downcase == '.mp4'
        @video = VideoBackground.new(path, @renderer.width, @renderer.height, @fps)
        @video_frame = (@time * @fps).floor - 1
        @background_buffer = FFI::MemoryPointer.new(:uchar, @renderer.width * @renderer.height * 4)
      else
        @background_texture = Native.check(Native.l2d_texture_load(@background_path))
      end
    end
    def clear_background
      @video&.close; @video = nil
      Native.l2d_texture_destroy(@background_texture) if @background_texture
      @background_texture = nil
    end

    def record_start(output, codec, fps = @record_fps)
      raise Error, 'Already recording' if @exporter
      raise Error, 'Wait for the previous recording to finish saving' if @finalizer&.alive?
      raise Error, 'Wait for audio preparation to complete' if @job_thread&.alive?
      if %w[h264 h265].include?(codec) && @color[3] < 1 && !@background_texture && !@video
        raise Error, 'Choose an opaque background for H.264/H.265 or select an alpha codec'
      end
      @record_fps = @offline ? @fps : Live2D.number(fps, min: 1, max: 120)
      @exporter = Exporter.new(output: output, width: @renderer.width, height: @renderer.height, fps: @record_fps,
        codec: codec, audio: @clip&.pcm_path, realtime: !@offline, queue_frames: @config.dig('export', 'queue_frames'))
      @record_start, @capture_count, @previous_capture = @time, 0, nil
      @gpu_dropped = 0
      @last_export = nil
      Native.l2d_capture_reset unless @offline
      @notice = @offline ? 'Rendering...' : 'Recording lossless alpha; delivery format saves after Stop.'
      command('op' => 'audio_play') if @clip
    end
    def capture
      return unless @exporter
      unless @offline
        drain_capture
        frame = ((@time - @record_start) * @record_fps).floor
        return if frame < @capture_count
        @gpu_dropped += [frame - @capture_count, 0].max
        @gpu_dropped += 1 unless @renderer.capture_submit(frame)
        @capture_count = frame + 1
        return
      end
      frame = @offline ? @capture_count : ((@time - @record_start) * @fps).floor
      return if frame < @capture_count
      pixels = @renderer.rgba
      while @capture_count < frame
        @exporter.push(@previous_capture || pixels)
        @capture_count += 1
      end
      @exporter.push(pixels)
      @previous_capture = pixels
      @capture_count += 1
    rescue StandardError => e
      @exporter.abort
      @last_export = @exporter.stats.merge('state' => 'failed')
      @exporter = nil
      Native.l2d_capture_reset
      @error = "Recording stopped: #{e.message}"
    end
    def drain_capture
      3.times do
        ready = @renderer.capture_poll
        break unless ready
        pixels, frame = ready
        @exporter.push(pixels, frame: frame)
      end
    end
    def record_stop
      raise Error, 'Not recording' unless @exporter
      drain_capture unless @offline
      exporter, @exporter = @exporter, nil
      if @offline
        output = exporter.finish
        @last_export = exporter.stats
        return output
      end
      Native.l2d_capture_reset
      total = [((@time - @record_start) * @record_fps).ceil, @capture_count].max
      @saving_exporter = exporter
      @notice = 'Saving final video in the background. The preview remains live.'
      @finalizer = Thread.new do
        exporter.finish(total_frames: total)
        @jobs << [:saved, exporter.stats.merge('missed_capture_ticks' => @gpu_dropped)]
      rescue StandardError => e
        @jobs << [:save_error, "#{e.message} (capture: #{exporter.intermediate})"]
      end
      { 'state' => 'saving', 'output' => exporter.output }
    end

    def snapshot(path)
      raise Error, 'Snapshot must have a .png extension and not exist' unless File.extname(path).downcase == '.png' && !File.exist?(path)
      FileUtils.mkdir_p(File.dirname(File.expand_path(path)))
      _, err, result = Open3.capture3(ENV.fetch('FFMPEG', 'ffmpeg'), '-v', 'error', '-n', '-f', 'rawvideo', '-pixel_format', 'rgba',
        '-video_size', "#{@renderer.width}x#{@renderer.height}", '-i', 'pipe:0', '-frames:v', '1', File.expand_path(path),
        stdin_data: @renderer.rgba, binmode: true)
      raise Error, "Snapshot failed: #{err}" unless result.success?
    end

    def refresh_elevenlabs_voices
      job(kind: :elevenlabs_voices) { @elevenlabs.fetch_voices }
    end
    def job(kind: :clip, &work)
      raise Error, 'An audio job is already running' if @job_thread&.alive?
      @notice = kind == :elevenlabs_voices ? 'Loading ElevenLabs voices...' : 'Preparing audio...'
      @error = ''
      @job_thread = Thread.new do
        @jobs << [kind, work.call]
      rescue StandardError => e
        @jobs << [kind == :elevenlabs_voices ? :elevenlabs_error : :error, e.message]
      end
    end
    def process_jobs
      return if @jobs.empty?
      kind, value = @jobs.pop
      case kind
      when :clip then load_clip(value)
      when :clip_play
        load_clip(value)
        command('op' => 'audio_play')
      when :elevenlabs_voices
        @elevenlabs.apply_voices(value)
        save_preferences
        @notice = @elevenlabs.message
      when :elevenlabs_error
        @elevenlabs.refresh_failed(value)
        @error = value
      when :saved
        @last_export, @saving_exporter = value, nil
        @notice = "Saved #{value['output']} (#{value['frames']} frames, #{value['duplicates']} repeated)."
      when :save_error
        @last_export = @saving_exporter&.stats
        @saving_exporter = nil
        @error = value
      else @error = value
      end
    end
    def load_clip(clip)
      warn "Audio ready for native load: #{clip.duration.round(3)}s" if ENV['L2D_TRACE'] == '1'
      if @exporter || @finalizer&.alive?
        clip.close
        raise Error, 'Audio finished preparing after recording began; load it again after recording'
      end
      if @audio_available && Native.l2d_audio_load(clip.pcm_path).zero?
        clip.close
        raise Error, 'Native audio playback could not load this file'
      end
      @clip&.close
      @clip, @audio_started = clip, nil
      warn 'Native audio load complete' if ENV['L2D_TRACE'] == '1'
      @notice = "Audio ready: #{clip.duration.round(2)} seconds. #{clip.cues.empty? ? 'Envelope / spectral proxy mode.' : 'Rhubarb visemes loaded.'}"
    end
    def status
      { 'mode' => @engine.mode, 'model' => @model ? File.basename(@model.path) : 'No model loaded',
        'model_path' => @model&.path,
        'agent' => { 'ready' => !!@model && @engine.mode == 'agent',
                     'commands' => @agent_commands,
                     'last_control_age' => @agent_last_control && (Live2D.monotonic - @agent_last_control).round(1) },
        'parameter_count' => @model&.info&.fetch('parameters')&.size || 0,
        'drawable_count' => @model&.info&.fetch('drawables')&.size || 0,
        'width' => @renderer.width, 'height' => @renderer.height,
        'render_frames' => @frames, 'time' => @time,
        'recording' => !!@exporter, 'calibration' => @engine.calibration.count,
        'record_fps' => @record_fps,
        'export' => (@exporter || @saving_exporter)&.stats || @last_export,
        'missed_capture_ticks' => @gpu_dropped || 0,
        'guides' => Guides.state(@guides),
        'ui_tab_request' => @ui_tab_request,
        'ui_settings' => @ui_settings,
        'voice' => @voice.status.merge('busy' => !!@job_thread&.alive?),
        'elevenlabs' => @elevenlabs.status.merge('busy' => !!@job_thread&.alive?),
        'connection_settings' => @connections&.settings || Connections::DEFAULTS,
        'connections' => @connections&.status || {},
        'notice' => @notice, 'error' => @error, 'fps' => @display_fps || 0,
        'audio' => @clip ? { 'duration' => @clip.duration, 'path' => @clip.path,
                             'native_playback' => !!@audio_available, 'visemes' => !@clip.cues.empty? } : nil,
        'parameters' => @model ? @model.info['parameters'].map { |p| p.merge('value' => (@model_parameters || {}).fetch(p['id'], p['default'])) } : [],
        'attachments' => @attachments.map { |a| a.slice(:name, :anchor, :scale, :rotation) } }
    end
    def ui(dt)
      @fps_wall_total = (@fps_wall_total || 0) + dt
      @fps_frame_count = (@fps_frame_count || 0) + 1
      if @fps_wall_total >= 0.5
        @display_fps = @fps_frame_count / @fps_wall_total
        @fps_wall_total, @fps_frame_count = 0.0, 0
      end
      events = JSON.parse(Native.l2d_ui_frame(@renderer.texture, @renderer.width, @renderer.height, JSON.generate(status)))
      if @options[:ui_snapshot] && @frames >= (@frame_limit ? @frame_limit - 1 : 2)
        path = @options.delete(:ui_snapshot)
        raise Error, 'UI snapshot already exists' if File.exist?(path)
        FileUtils.mkdir_p(File.dirname(File.expand_path(path)))
        raise Error, 'UI snapshot failed' if Native.l2d_ui_snapshot(File.expand_path(path)).zero?
      end
      events.each { |c| @inbox.push(c) }
      GLFW.glfwSwapBuffers(@window)
    end
    def install_drop
      @drop_callback = proc do |_window, count, paths|
        xp, yp = FFI::MemoryPointer.new(:double), FFI::MemoryPointer.new(:double)
        GLFW.glfwGetCursorPos(@window, xp, yp)
        point = FFI::MemoryPointer.new(:float, 2)
        inside = Native.l2d_ui_canvas_point(xp.read_double, yp.read_double, point) != 0
        x, y = inside ? point.read_array_of_float(2) : [0, 0]
        primary_assigned = !@model.nil?
        paths.read_array_of_pointer(count).each do |ptr|
          path = ptr.read_string.force_encoding('UTF-8')
          if path.end_with?('.model3.json', '.moc3')
            @inbox.push('op' => 'load_model', 'path' => path, 'primary' => !primary_assigned, 'x' => x, 'y' => y)
            primary_assigned = true
          elsif path.match?(/\.(wav|mp3)\z/i)
            @inbox.push('op' => 'audio', 'path' => path)
          elsif path.match?(/\.png\z/i)
            @inbox.push('op' => 'prop', 'path' => path, 'x' => x, 'y' => y)
          else
            @error = 'Drop a .model3.json/.moc3, PNG prop, WAV or MP3.'
          end
        end
      rescue StandardError => e
        @error = e.message
      end
      GLFW.glfwSetDropCallback(@window, @drop_callback)
    end
    def dispose_attachment(a)
      a[:model]&.close
      Native.l2d_texture_destroy(a[:texture]) if a[:texture]
    end
    def save_preferences
      @preferences['guides'] = @guides
      @preferences['ui'] = @ui_settings
      @preferences['voice'] = @voice.settings
      @preferences['elevenlabs'] = @elevenlabs.settings
      @preferences['connections'] = @connections.settings if @connections
      File.write(@settings_path, JSON.pretty_generate(@preferences))
    end
    def close
      @connections&.close
      @network&.stop
      @exporter&.abort
      @saving_exporter&.abort if @finalizer&.alive?
      @finalizer&.join(2)
      @job_thread&.join(2)
      clear_background
      Native.l2d_audio_shutdown
      @clip&.close
      @attachments.each { |a| dispose_attachment(a) }
      @model&.close
      @renderer&.close
      Native.l2d_ui_shutdown unless @options[:hidden]
      Native.l2d_shutdown
      GLFW.glfwDestroyWindow(@window) if @window && !@window.null?
      GLFW.glfwTerminate
    end
  end
end
