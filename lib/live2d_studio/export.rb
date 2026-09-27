# frozen_string_literal: true
module Live2D
  class Exporter
    CODECS = {
      'h264' => ['.mp4', %w[-c:v libx264 -crf 16 -preset medium -pix_fmt yuv420p -movflags +faststart]],
      'h265' => ['.mp4', %w[-c:v libx265 -crf 18 -pix_fmt yuv420p -tag:v hvc1]],
      'prores' => ['.mov', %w[-c:v prores_ks -profile:v 4 -pix_fmt yuva444p10le -alpha_bits 16]],
      'vp9' => ['.webm', %w[-c:v libvpx-vp9 -pix_fmt yuva420p -b:v 0 -crf 18 -auto-alt-ref 0 -metadata:s:v:0 alpha_mode=1]]
    }.freeze
    attr_reader :frames, :dropped, :output, :state, :duplicates, :captured, :intermediate

    def initialize(output:, width:, height:, fps:, codec:, audio: nil, realtime: false,
      queue_frames: 4, ffmpeg: ENV.fetch('FFMPEG', 'ffmpeg'))
      @output = File.expand_path(output)
      ext, options = CODECS.fetch(codec) { raise Error, 'Unknown export codec' }
      raise Error, "#{codec} requires #{ext}" unless File.extname(output).downcase == ext
      raise Error, 'Export dimensions must be even and between 16 and 8192' unless [width, height].all? { |n| n.is_a?(Integer) && n.between?(16, 8192) && n.even? }
      Live2D.number(fps, min: 1, max: 120)
      raise Error, 'Output already exists; choose a new filename' if File.exist?(@output)
      FileUtils.mkdir_p(File.dirname(@output))
      @frame_bytes, @frames, @dropped = width * height * 4, 0, 0
      @realtime = realtime
      @duplicates, @captured, @next_index = 0, 0, 0
      @state = 'recording'
      @progress = 0.0
      @ffmpeg, @fps, @codec, @audio = ffmpeg, fps, codec, audio && File.expand_path(audio)
      @partial = @output.sub(/#{Regexp.escape(ext)}\z/, ".partial#{ext}")
      raise Error, 'Partial output already exists' if File.exist?(@partial)
      # Live frames go to a fast, lossless alpha intermediate. Expensive delivery
      # compression starts only after capture ends, outside the application loop.
      @intermediate = "#{@output}.capture.mkv" if @realtime
      raise Error, 'Capture intermediate already exists' if @intermediate && File.exist?(@intermediate)
      command = [ffmpeg, '-v', 'warning', '-nostdin', '-n', '-f', 'rawvideo', '-pixel_format', 'rgba',
        '-video_size', "#{width}x#{height}", '-framerate', fps.to_s, '-i', 'pipe:0']
      command += ['-map', '0:v:0', *(@realtime ? %w[-c:v utvideo -pix_fmt gbrap -pred left -threads 4] : options)]
      command << (@intermediate || @partial)
      @stdin, @stdout, @stderr, @wait = Open3.popen3(*command)
      @stdin.binmode
      @log = +''
      @drain = Thread.new { @stderr.each_line { |line| @log << line; @log = @log[-16_384..] if @log.size > 16_384 } }
      @out_drain = Thread.new { @stdout.read }
      @queue = SizedQueue.new(queue_frames)
      @worker = Thread.new do
        previous = nil
        while (item = @queue.pop)
          frame, index = item
          while @frames < index
            @stdin.write(previous || frame)
            @frames += 1
            @duplicates += 1
          end
          @stdin.write(frame)
          @frames += 1
          previous = frame
        end
        while previous && @target_frames && @frames < @target_frames
          @stdin.write(previous)
          @frames += 1
          @duplicates += 1
        end
      rescue StandardError => e
        @failure = e
      ensure
        @stdin.close rescue nil
      end
    end

    def push(rgba, frame: nil)
      raise Error, 'Invalid RGBA frame length' unless rgba.bytesize == @frame_bytes
      raise Error, "FFmpeg pipe failed: #{@failure.message}" if @failure
      index = frame || @next_index
      raise Error, 'Frame indices must increase' unless index.is_a?(Integer) && index >= @next_index
      @next_index = index + 1
      loop do
        raise Error, 'FFmpeg exited before frame submission' unless @worker.alive?
        begin
          @queue.push([rgba, index], true)
          @captured += 1
          return true
        rescue ThreadError
          if @realtime
            @dropped += 1
            return false
          end
          sleep(0.001)
        end
      end
    end

    def stats
      { 'state' => @state, 'frames' => @frames, 'captured' => @captured, 'queue' => @queue.size,
        'dropped' => @dropped, 'duplicates' => @duplicates, 'output' => @output, 'progress' => @progress }
    end

    def finish(total_frames: nil)
      @state = 'draining'
      @target_frames = total_frames
      @queue.close
      raise Error, 'Encoder did not drain within 120 seconds' unless @worker.join(120)
      raise Error, 'Encoder did not exit within 120 seconds' unless @wait.join(120)
      @drain.join(2)
      @out_drain.join(2)
      raise Error, "FFmpeg failed: #{@log}" if @failure || !@wait.value.success? || @frames.zero?
      if @realtime || @audio
        @state = 'saving'
        muxed = @realtime ? @partial : @partial.sub('.partial', '.mux.partial')
        duration = format('%.9f', @frames.to_f / @fps)
        audio_codec = @codec == 'vp9' ? 'libopus' : (@codec == 'prores' ? 'pcm_s16le' : 'aac')
        args = [@ffmpeg, '-v', 'error', '-nostdin', '-n', '-progress', 'pipe:1', '-i', @intermediate || @partial]
        args += ['-i', @audio] if @audio
        args += ['-map', '0:v:0', *(@realtime ? CODECS.fetch(@codec)[1] : ['-c:v', 'copy'])]
        args += %w[-deadline good -cpu-used 4 -row-mt 1] if @realtime && @codec == 'vp9'
        args += ['-map', '1:a:0', '-c:a', audio_codec, '-af', "apad,atrim=end=#{duration},asetpts=PTS-STARTPTS"] if @audio
        args += ['-t', duration, muxed]
        Open3.popen3(*args) do |input, out, err, wait|
          @final_wait = wait
          input.close
          drain = Thread.new do
            out.each_line do |line|
              key, value = line.strip.split('=', 2)
              @progress = (value.to_f / 1_000_000 / duration.to_f).clamp(0, 1) if key == 'out_time_us'
            end
          end
          stderr = err.read
          drain.join
          raise Error, "Final export failed: #{stderr}. Capture preserved: #{@intermediate}" unless wait.value.success?
        end
        File.rename(muxed, @output)
        FileUtils.rm_f(@partial)
        FileUtils.rm_f(@intermediate) if @intermediate
      else
        File.rename(@partial, @output)
      end
      @state = 'saved'
      @progress = 1.0
      @output
    rescue StandardError
      @state = 'failed'
      abort
      raise
    end

    def abort
      @queue&.close
      Process.kill('KILL', @wait.pid) if @wait&.alive?
      Process.kill('KILL', @final_wait.pid) if @final_wait&.alive?
      @worker&.join(2)
      @stdin&.close rescue nil
      @wait&.join(2)
      @drain&.join(2)
    rescue Errno::ESRCH
      nil
    end
  end

  class VideoBackground
    def initialize(path, width, height, fps, ffmpeg: ENV.fetch('FFMPEG', 'ffmpeg'))
      @size = width * height * 4
      @stdin, @out, @err, @wait = Open3.popen3(ffmpeg, '-v', 'error', '-nostdin', '-stream_loop', '-1',
        '-i', File.expand_path(path), '-vf', "scale=#{width}:#{height}:force_original_aspect_ratio=increase,crop=#{width}:#{height},fps=#{fps}",
        '-f', 'rawvideo', '-pix_fmt', 'rgba', 'pipe:1')
      @stdin.close
      @out.binmode
      @drain = Thread.new { @err.read }
    end
    def frame
      bytes = @out.read(@size)
      raise Error, 'Background video decode ended unexpectedly' unless bytes&.bytesize == @size
      bytes
    end
    def close
      @out.close rescue nil
      Process.kill('KILL', @wait.pid) if @wait.alive?
      @wait.join(2)
      @drain.join(2)
    rescue Errno::ESRCH
      nil
    end
  end
end
