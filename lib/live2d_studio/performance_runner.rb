# frozen_string_literal: true
require 'rbconfig'

module Live2D
  class PerformanceRunner
    class Client
      def initialize
        port = Integer(ENV.fetch('L2D_API_PORT', '4141'))
        token_file = File.join(ROOT, 'tmp', port == 4141 ? 'api-token' : "api-token-#{port}")
        token = ENV['L2D_API_TOKEN'].to_s
        token = File.read(token_file).strip if token.empty?
        @token = token
        @socket = TCPSocket.new('127.0.0.1', port)
      end

      def call(command)
        @socket.write(JSON.generate(command.merge('token' => @token, 'id' => SecureRandom.hex(5))) + "\n")
        raise Error, 'Studio API timed out' unless IO.select([@socket], nil, nil, 20)
        response = JSON.parse(@socket.gets || raise(Error, 'Studio disconnected'))
        raise Error, response.fetch('error', 'Studio command failed') unless response['ok']
        response['result']
      end

      def close = @socket&.close
    end

    def initialize(plan, script_path:, output:, ffmpeg: nil, ruby: RbConfig.ruby)
      @plan = plan
      @script_path = File.expand_path(script_path)
      @output = File.expand_path(output)
      @ruby = ruby
      @ffmpeg = ffmpeg || ENV['FFMPEG'] || Dir.glob(File.join(ROOT, '.tools', 'ffmpeg*', 'bin',
        Gem.win_platform? ? 'ffmpeg.exe' : 'ffmpeg')).first || 'ffmpeg'
      raise Error, 'Output must be a new .mp4 file' unless File.extname(@output).downcase == '.mp4' && !File.exist?(@output)
      @work = @output.sub(/\.mp4\z/i, '.performance')
      raise Error, 'Performance work folder already exists; choose a new output name' if File.exist?(@work)
    end

    def run
      client = Client.new
      state = client.call('op' => 'status')
      model = @plan.model_path ? File.expand_path(@plan.model_path, File.dirname(@script_path)) : state['model_path']
      raise Error, 'Load a model in Valkyrie Studio or set model_path in the script' unless model && File.file?(model)
      if @plan.provider != 'system' && !state.dig(@plan.provider == 'openai' ? 'voice' : 'elevenlabs', 'configured')
        raise Error, "Configure #{@plan.provider} speech in Valkyrie Studio first"
      end
      FileUtils.mkdir_p(File.dirname(@output))
      Dir.mkdir(@work)
      puts "Preparing #{@plan.lines.length} voice lines with #{@plan.provider}..."
      sample_counts = @plan.lines.each_with_index.map do |line, index|
        source = synthesize(client, line, index)
        raw = File.join(@work, format('line-%04d.raw.pcm', index + 1))
        pcm = File.join(@work, format('line-%04d.pcm', index + 1))
        run_command(@ffmpeg, '-v', 'error', '-nostdin', '-n', '-i', source,
                    '-ac', '1', '-ar', Performance::RATE.to_s, '-c:a', 'pcm_s16le', '-f', 's16le', raw)
        trim_silent_edges(raw, pcm)
        count = File.size(pcm) / 2
        puts format('Voice line %d/%d: %.2fs', index + 1, @plan.lines.length, count.to_f / Performance::RATE)
        count
      end
      chapters = @plan.chapters(sample_counts)
      chapter_files = chapters.each_with_index.map do |chapter, index|
        base = format('chapter-%03d', index + 1)
        audio = File.join(@work, "#{base}.wav")
        write_audio(chapter, audio)
        timeline = File.join(@work, "#{base}.json")
        File.write(timeline, JSON.generate(@plan.timeline(chapter)))
        video = chapters.length == 1 ? @output : File.join(@work, "#{base}.mp4")
        render(model, audio, timeline, video, chapter, index, chapters.length)
        video
      end
      join_chapters(chapter_files, chapters) if chapter_files.length > 1
      File.write(File.join(@work, 'timing.json'), JSON.pretty_generate({
        'script' => @script_path, 'output' => @output, 'model' => model,
        'provider' => @plan.provider, 'fps' => @plan.fps,
        'chapters' => chapters.map.with_index do |chapter, index|
          { 'file' => File.basename(chapter_files[index]), 'duration' => chapter[:samples].to_f / Performance::RATE,
            'lines' => chapter[:entries].map { |entry| { 'index' => entry[:index] + 1,
              'start' => entry[:start_sample].to_f / Performance::RATE,
              'speech_duration' => entry[:speech_samples].to_f / Performance::RATE } } }
        end
      }))
      puts "Saved #{@output} (#{chapters.length} synchronized chapter#{chapters.length == 1 ? '' : 's'})"
      @output
    ensure
      client&.close
    end

    def synthesize(client, line, index)
      previous = client.call('op' => 'status').dig('audio', 'path')
      command = { 'op' => 'tts', 'provider' => @plan.provider, 'text' => line['text'], 'autoplay' => false }
      command['voice'] = line['voice'] if line['voice']
      command['model'] = line['tts_model'] if line['tts_model']
      client.call(command)
      deadline = Live2D.monotonic + 180
      loop do
        state = client.call('op' => 'status')
        raise Error, "Voice line #{index + 1}: #{state['error']}" unless state['error'].to_s.empty?
        path = state.dig('audio', 'path')
        return path if path && path != previous && !state.dig(@plan.provider == 'openai' ? 'voice' : 'elevenlabs', 'busy') && File.file?(path)
        raise Error, "Voice line #{index + 1} timed out" if Live2D.monotonic > deadline
        sleep 0.25
      end
    end

    def write_audio(chapter, output)
      raw = output.sub(/\.wav\z/, '.pcm')
      File.open(raw, 'wb') do |writer|
        silence(writer, chapter[:lead_samples])
        chapter[:entries].each do |entry|
          File.open(File.join(@work, format('line-%04d.pcm', entry[:index] + 1)), 'rb') do |reader|
            IO.copy_stream(reader, writer)
          end
          silence(writer, entry[:pause_samples])
        end
        silence(writer, chapter[:tail_samples])
      end
      raise Error, 'Chapter audio sample count changed' unless File.size(raw) == chapter[:samples] * 2
      run_command(@ffmpeg, '-v', 'error', '-nostdin', '-n', '-f', 's16le', '-ar', Performance::RATE.to_s,
                  '-ac', '1', '-i', raw, '-c:a', 'pcm_s16le', output)
    end

    # Remove only quiet padding at the beginning/end of a generated line.
    # Internal breaths and pauses remain intact; a 50 ms guard protects words.
    def trim_silent_edges(source, destination)
      samples = File.size(source) / 2
      raise Error, 'Voice line has no decoded samples' if samples.zero?
      window = 480
      scan_limit = [samples / window, 80].min
      front_windows = 0
      back_windows = 0
      File.open(source, 'rb') do |reader|
        scan_limit.times do |i|
          reader.seek(i * window * 2)
          break unless quiet?(reader.read(window * 2))
          front_windows += 1
        end
        scan_limit.times do |i|
          reader.seek((samples - (i + 1) * window) * 2)
          break unless quiet?(reader.read(window * 2))
          back_windows += 1
        end
      end
      guard = 5 * window
      front = [front_windows * window - guard, 0].max
      back = [back_windows * window - guard, 0].max
      raise Error, 'Voice line is silent' if front + back >= samples
      File.open(source, 'rb') do |reader|
        File.open(destination, 'wb') do |writer|
          reader.seek(front * 2)
          IO.copy_stream(reader, writer, (samples - front - back) * 2)
        end
      end
    end

    def quiet?(bytes)
      values = bytes.unpack('s<*')
      values.empty? || values.sum { |value| value * value }.to_f / values.length < (32768 * 0.001)**2
    end

    def silence(writer, samples)
      chunk = "\0" * 65_536
      bytes = samples * 2
      while bytes.positive?
        size = [bytes, chunk.bytesize].min
        writer.write(chunk.byteslice(0, size))
        bytes -= size
      end
    end

    def render(model, audio, timeline, video, chapter, index, total)
      puts format('Rendering chapter %d/%d (%.2fs)...', index + 1, total, chapter[:samples].to_f / Performance::RATE)
      report = File.join(@work, format('chapter-%03d-report.json', index + 1))
      studio = File.join(ROOT, 'bin', 'studio')
      env = { 'FFMPEG' => @ffmpeg }
      run_command(env, @ruby, studio, '--model', model, '--audio', audio, '--timeline', timeline,
                  '--width', @plan.width.to_s, '--height', @plan.height.to_s,
                  '--fps', @plan.fps.to_s, '--codec', 'h264', '--output', video,
                  '--report', report, '--hidden')
      result = JSON.parse(File.read(report))
      raise Error, "Chapter #{index + 1} export did not finish" unless result.dig('export', 'state') == 'saved'
    end

    def join_chapters(files, chapters)
      manifest = File.join(@work, 'chapters.ffconcat')
      File.write(manifest, "ffconcat version 1.0\n" + files.map { |path| "file '#{File.basename(path)}'\n" }.join)
      partial = @output.sub(/\.mp4\z/i, '.partial.mp4')
      raise Error, 'Partial output already exists' if File.exist?(partial)
      total_frames = chapters.sum { |chapter| (chapter[:samples].to_f / Performance::RATE * @plan.fps).ceil }
      duration = format('%.9f', total_frames.to_f / @plan.fps)
      run_command(@ffmpeg, '-v', 'error', '-nostdin', '-n', '-safe', '0', '-f', 'concat',
                  '-i', manifest, '-map', '0:v:0', '-map', '0:a:0', '-c:v', 'copy',
                  '-c:a', 'aac', '-b:a', '192k', '-af', "apad,atrim=end=#{duration},asetpts=PTS-STARTPTS",
                  '-t', duration, '-movflags', '+faststart', partial)
      raise Error, 'Output appeared during rendering; choose another name' if File.exist?(@output)
      File.rename(partial, @output)
    end

    def run_command(*command)
      _stdout, stderr, status = Open3.capture3(*command)
      raise Error, "Command failed: #{stderr[-2000, 2000] || stderr}" unless status.success?
    end
  end
end
