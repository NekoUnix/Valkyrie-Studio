# frozen_string_literal: true
require 'net/http'
require 'uri'
require 'cgi'

module Live2D
  module Audio
    # All ingest is converted to canonical 48 kHz mono PCM. Ruby evaluates mouth
    # curves at the same sample time used by miniaudio playback / offline export.
    class Clip
      RATE = 48_000
      VISEMES = { 'A' => [0.05, 0.0], 'B' => [0.2, 0.35], 'C' => [0.55, 0.65],
        'D' => [0.9, 0.15], 'E' => [0.5, -0.65], 'F' => [0.25, -0.85],
        'G' => [0.2, 0.0], 'H' => [0.3, 0.4], 'X' => [0.0, 0.0] }.freeze
      attr_reader :path, :duration, :cues, :samples, :pcm_path

      def initialize(path, ffmpeg: nil, cues_path: nil)
        ffmpeg ||= ENV['FFMPEG'] || Dir.glob(File.join(ROOT, '.tools', 'ffmpeg*', 'bin',
          Gem.win_platform? ? 'ffmpeg.exe' : 'ffmpeg')).first || 'ffmpeg'
        @path = File.expand_path(path)
        raise Error, 'Audio file does not exist' unless File.file?(@path)
        FileUtils.mkdir_p(File.join(ROOT, 'tmp'))
        @pcm_path = File.join(ROOT, 'tmp', "#{SecureRandom.hex(8)}.wav")
        _, stderr, status = Open3.capture3(ffmpeg, '-v', 'error', '-nostdin', '-y', '-i', @path,
          '-t', '600.01', '-ac', '1', '-ar', RATE.to_s, '-c:a', 'pcm_s16le', @pcm_path)
        raise Error, "Audio decode failed: #{stderr[-2000..] || stderr}" unless status.success?
        pcm, stderr, status = Open3.capture3(ffmpeg, '-v', 'error', '-nostdin', '-i', @pcm_path,
          '-f', 's16le', '-acodec', 'pcm_s16le', '-', binmode: true)
        raise Error, "PCM read failed: #{stderr}" unless status.success?
        @samples = pcm.unpack('s<*')
        @duration = @samples.length.to_f / RATE
        raise Error, 'Audio is longer than the supported 10 minutes; trim it before loading' if @duration > 600
        @cues = cues_path ? JSON.parse(File.read(cues_path)).fetch('mouthCues') : []
        @cues.each do |cue|
          Live2D.number(cue.fetch('start'), min: 0, max: @duration + 1)
          Live2D.number(cue.fetch('end'), min: cue['start'], max: @duration + 1)
          raise Error, 'Unknown Rhubarb viseme' unless VISEMES.key?(cue.fetch('value'))
        end
        @cues.sort_by! { |cue| cue['start'] }
        @envelopes = build_envelopes
      rescue StandardError
        FileUtils.rm_f(@pcm_path) if @pcm_path
        raise
      end

      def build_envelopes
        @samples.each_slice(480).map do |window|
          rms = Math.sqrt(window.sum { |x| (x / 32768.0)**2 } / window.size)
          crossings = window.each_cons(2).count { |a, b| (a < 0) != (b < 0) }.to_f / window.size
          [(rms * 5).clamp(0, 1), ((crossings - 0.07) * 8).clamp(-0.7, 0.7)]
        end
      end

      def mouth(time)
        return { 'ParamMouthOpenY' => 0.0, 'ParamMouthForm' => 0.0 } unless time.between?(0, @duration)
        index = time * 100
        a = @envelopes[index.floor] || [0, 0]
        b = @envelopes[index.floor + 1] || a
        frac = index - index.floor
        amplitude, form = a.zip(b).map { |x, y| x + (y - x) * frac }
        cue_index = @cues.bsearch_index { |c| c['end'] >= time }
        cue = cue_index && @cues[cue_index]
        if cue && cue['start'] <= time
          opening, form = VISEMES.fetch(cue['value'])
          # Short coarticulation transition at cue boundaries.
          previous = cue_index.positive? ? VISEMES.fetch(@cues[cue_index - 1]['value']) : [0, 0]
          mix = ((time - cue['start']) / 0.035).clamp(0, 1)
          opening = previous[0] + (opening - previous[0]) * mix
          form = previous[1] + (form - previous[1]) * mix
          amplitude = opening * [amplitude * 1.5, 1].min
        end
        { 'ParamMouthOpenY' => amplitude, 'ParamMouthForm' => form }
      end

      def close
        FileUtils.rm_f(@pcm_path)
      end
    end

    class TTS
      def synthesize(provider:, text:, output:, voice: nil, api_key: nil, model: nil, instructions: nil, speed: 1.0)
        raise Error, 'Speech text must contain 1–4000 characters' unless text.is_a?(String) && text.size.between?(1, 4000)
        case provider
        when 'openai'
          model ||= ENV.fetch('OPENAI_TTS_MODEL', 'gpt-4o-mini-tts')
          payload = { model: model, input: text, voice: voice || ENV.fetch('OPENAI_TTS_VOICE', 'coral'),
            speed: Live2D.number(speed, min: 0.25, max: 4), response_format: 'wav' }
          payload[:instructions] = instructions if model.start_with?('gpt-4o-mini-tts') && instructions && !instructions.empty?
          post('https://api.openai.com/v1/audio/speech',
            { 'Authorization' => "Bearer #{api_key || key('OPENAI_API_KEY')}" }, payload, output)
        when 'elevenlabs'
          id = voice || key('ELEVENLABS_VOICE_ID')
          raise Error, 'Invalid voice ID' unless id.match?(/\A[A-Za-z0-9_-]+\z/)
          post("https://api.elevenlabs.io/v1/text-to-speech/#{id}?output_format=mp3_44100_128",
            { 'xi-api-key' => api_key || key('ELEVENLABS_API_KEY') },
            { text: text, model_id: model || ENV.fetch('ELEVENLABS_MODEL', 'eleven_multilingual_v2') }, output)
        when 'system'
          system_speech(text, output, voice)
        else
          raise Error, 'TTS provider must be openai, elevenlabs or system'
        end
        output
      end

      def key(name)
        ENV.fetch(name) { raise Error, "Set #{name} in .env" }.tap { |v| raise Error, "Set #{name}" if v.empty? }
      end

      def post(url, headers, body, output)
        uri = URI(url)
        request = Net::HTTP::Post.new(uri, headers.merge('Content-Type' => 'application/json'))
        request.body = JSON.generate(body)
        Net::HTTP.start(uri.host, uri.port, use_ssl: true, open_timeout: 10, read_timeout: 90) do |http|
          http.request(request) do |response|
            unless response.is_a?(Net::HTTPSuccess)
              message = { '401' => 'API key was rejected. Replace the configured provider key.',
                '403' => 'This API key or project does not have speech access.',
                '429' => 'API quota or rate limit reached. Check your provider API billing and limits.',
                '400' => 'Speech settings were rejected. Check the model, voice and text.' }.fetch(response.code, 'Speech service request failed. Try again shortly.')
              raise Error, "#{message} (HTTP #{response.code})"
            end
            bytes = 0
            File.open(output, 'wb') do |file|
              response.read_body do |chunk|
                bytes += chunk.bytesize
                raise Error, 'TTS response exceeds 64 MiB' if bytes > 64 * 1024 * 1024
                file.write(chunk)
              end
            end
          end
        end
      rescue StandardError
        FileUtils.rm_f(output)
        raise
      end

      def system_speech(text, output, voice)
        if RUBY_PLATFORM.match?(/mingw|mswin/)
          script = File.join(ROOT, 'scripts', 'speak.ps1')
          command = ['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', script, '-OutputPath', File.expand_path(output)]
          command += ['-Voice', voice] if voice
        elsif RUBY_PLATFORM.include?('darwin')
          command = ['say', '-o', output, '--file=-', '--file-format=WAVE', '--data-format=LEI16@48000']
          command += ['-v', voice] if voice
        else
          command = ['espeak-ng', '-w', output, '--stdin']
          command += ['-v', voice] if voice
        end
        _, error, status = Open3.capture3(*command, stdin_data: text)
        raise Error, "System speech failed: #{error}" unless status.success?
      end
    end
  end
end
