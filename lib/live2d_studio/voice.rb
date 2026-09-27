# frozen_string_literal: true
module Live2D
  module Audio
    # Windows DPAPI binds the saved credential to the interactive Windows user.
    # Other platforms support environment/.env keys or a session-only UI key.
    class KeyStore
      def initialize(path = File.join(ROOT, '.credentials', 'openai-key.dpapi'))
        @path = path
      end
      def persistent? = Gem.win_platform?
      def read
        return nil unless persistent? && File.file?(@path)
        transform(File.binread(@path), false)
      end
      def write(key)
        raise Error, 'Persistent key storage is available on Windows; use session mode or OPENAI_API_KEY here' unless persistent?
        encrypted = transform(key, true)
        FileUtils.mkdir_p(File.dirname(@path))
        File.binwrite(@path, encrypted)
      end
      def delete = FileUtils.rm_f(@path)

      def transform(bytes, protect)
        require 'ffi'
        unless self.class.const_defined?(:WinCrypto, false)
          self.class.const_set(:WinCrypto, Module.new do
            extend FFI::Library
            ffi_lib 'crypt32'
            attach_function :CryptProtectData, [:pointer, :pointer, :pointer, :pointer, :pointer, :uint, :pointer], :int
            attach_function :CryptUnprotectData, [:pointer, :pointer, :pointer, :pointer, :pointer, :uint, :pointer], :int
            ffi_lib 'kernel32'
            attach_function :LocalFree, [:pointer], :pointer
            attach_function :GetLastError, [], :uint
          end)
          self.class.const_set(:Blob, Class.new(FFI::Struct) { layout :size, :uint, :data, :pointer })
        end
        input, output = self.class::Blob.new, self.class::Blob.new
        buffer = FFI::MemoryPointer.new(:uchar, bytes.bytesize)
        buffer.put_bytes(0, bytes)
        input[:size], input[:data] = bytes.bytesize, buffer
        crypto = self.class::WinCrypto
        method = protect ? :CryptProtectData : :CryptUnprotectData
        ok = crypto.public_send(method, input.pointer, nil, nil, nil, nil, 1, output.pointer)
        raise Error, "Windows credential storage is unavailable (#{crypto.GetLastError}). Use session mode or enter the key under your normal Windows account." if ok.zero?
        output[:data].read_bytes(output[:size])
      ensure
        crypto.LocalFree(output[:data]) if output && crypto && !output[:data].null?
        buffer&.clear
      end
    end

    class Voice
      VOICES = %w[alloy ash ballad coral echo fable nova onyx sage shimmer verse marin cedar].freeze
      MODELS = %w[gpt-4o-mini-tts gpt-4o-mini-tts-2025-12-15 tts-1 tts-1-hd].freeze
      DEFAULTS = { 'model' => 'gpt-4o-mini-tts', 'voice' => 'coral', 'speed' => 1.0,
        'instructions' => 'Speak naturally, warmly and clearly.', 'autoplay' => true }.freeze
      attr_reader :settings, :message
      def initialize(settings = {}, store: KeyStore.new)
        @store = store
        @settings = validate(DEFAULTS.merge({ 'model' => ENV.fetch('OPENAI_TTS_MODEL', DEFAULTS['model']),
          'voice' => ENV.fetch('OPENAI_TTS_VOICE', DEFAULTS['voice']) }).merge(settings))
        @api_key = ENV['OPENAI_API_KEY'].to_s.strip
        @api_key = @store.read.to_s if @api_key.empty?
        @message = configured? ? 'Key available; generate a preview to test voice access.' : 'Add your OpenAI API key to enable voice.'
      rescue Error => e
        @api_key = ''
        @message = e.message
      end
      def configured? = !@api_key.to_s.empty?
      def validate(values)
        result = DEFAULTS.merge(values.slice(*DEFAULTS.keys))
        raise Error, 'Unsupported speech model' unless MODELS.include?(result['model'])
        raise Error, 'Unknown speech voice' unless VOICES.include?(result['voice'])
        if %w[tts-1 tts-1-hd].include?(result['model']) && %w[ballad verse marin cedar].include?(result['voice'])
          raise Error, 'This voice needs gpt-4o-mini-tts'
        end
        result['speed'] = Live2D.number(result['speed'], min: 0.25, max: 4)
        raise Error, 'Voice direction must be text, at most 2000 characters' unless result['instructions'].is_a?(String) && result['instructions'].length <= 2000
        result['autoplay'] = !!result['autoplay']
        result
      end
      def configure(values)
        @settings = validate(@settings.merge(values))
      end
      def set_key(key, remember: true)
        raise Error, 'Enter an OpenAI API key' unless key.is_a?(String) && key.strip.match?(/\Ask-[A-Za-z0-9_-]{12,}\z/)
        clean = key.strip
        @store.write(clean) if remember
        @store.delete unless remember
        @api_key = clean
        @message = 'Key saved. Generate a preview to test speech access.'
      end
      def forget_key
        @store.delete
        @api_key = ''
        @message = 'Saved key removed; voice is disconnected for this session.'
      end
      def status
        @settings.merge('configured' => configured?, 'message' => @message,
          'persistent_storage' => @store.persistent?, 'voices' => VOICES, 'models' => MODELS)
      end
      def synthesize(text:, output:, settings: @settings, tts: TTS.new)
        raise Error, 'Add an OpenAI API key in Voice settings first' unless configured?
        options = validate(settings)
        tts.synthesize(provider: 'openai', text: text, output: output, api_key: @api_key,
          model: options['model'], voice: options['voice'], speed: options['speed'], instructions: options['instructions'])
        @message = 'OpenAI voice connected; speech generated successfully.'
        output
      end
    end

    class ElevenLabs
      DEFAULTS = { 'voice_id' => '', 'model' => 'eleven_multilingual_v2', 'autoplay' => true }.freeze
      attr_reader :settings, :message

      def initialize(settings = {}, store: KeyStore.new(File.join(ROOT, '.credentials', 'elevenlabs-key.dpapi')))
        @store = store
        @settings = validate(DEFAULTS.merge(settings))
        @api_key = ENV['ELEVENLABS_API_KEY'].to_s.strip
        @api_key = @store.read.to_s if @api_key.empty?
        @voices = []
        @message = configured? ? 'Key available. Refresh voices to connect.' : 'Add your ElevenLabs API key.'
      rescue Error => e
        @api_key = ''
        @message = e.message
      end

      def configured? = !@api_key.to_s.empty?
      def validate(values)
        result = DEFAULTS.merge(values.slice(*DEFAULTS.keys))
        id = result['voice_id']
        raise Error, 'Invalid ElevenLabs voice ID' unless id.is_a?(String) && (id.empty? || id.match?(/\A[A-Za-z0-9_-]+\z/))
        raise Error, 'Invalid ElevenLabs model ID' unless result['model'].is_a?(String) && result['model'].match?(/\A[A-Za-z0-9_-]{1,80}\z/)
        result['autoplay'] = !!result['autoplay']
        result
      end

      def configure(values)
        @settings = validate(@settings.merge(values))
      end

      def set_key(key, remember: true)
        raise Error, 'Enter an ElevenLabs API key' unless key.is_a?(String) && key.strip.match?(/\A\S{16,512}\z/)
        clean = key.strip
        @store.write(clean) if remember
        @store.delete unless remember
        @api_key = clean
        @voices = []
        @message = 'Key saved. Refresh voices to connect.'
      end

      def forget_key
        @store.delete
        @api_key = ''
        @voices = []
        @message = 'Saved key removed; ElevenLabs is disconnected.'
      end

      def status
        @settings.merge('configured' => configured?, 'message' => @message,
          'persistent_storage' => @store.persistent?, 'voices' => @voices)
      end

      def fetch_voices
        raise Error, 'Add your ElevenLabs API key first' unless configured?
        voices, token, seen, complete = [], nil, {}, false
        50.times do
          page = get_page(token)
          raise Error, 'ElevenLabs returned an invalid voice list' unless page.is_a?(Hash) && page['voices'].is_a?(Array)
          page['voices'].each do |voice|
            next unless voice.is_a?(Hash)
            id, name = voice.values_at('voice_id', 'name')
            next unless id.is_a?(String) && id.match?(/\A[A-Za-z0-9_-]+\z/) && name.is_a?(String) && !name.empty?
            next if seen[id]
            seen[id] = true
            voices << { 'voice_id' => id, 'name' => name }
          end
          unless page['has_more']
            complete = true
            break
          end
          token = page['next_page_token']
          raise Error, 'ElevenLabs voice pagination failed' unless token.is_a?(String) && !token.empty?
          raise Error, 'ElevenLabs voice list is too large' if voices.length > 5000
        end
        raise Error, 'ElevenLabs voice list exceeded 50 pages' unless complete
        voices.sort_by { |voice| [voice['name'].downcase, voice['voice_id']] }
      end

      def apply_voices(voices)
        @voices = voices
        ids = voices.map { |v| v['voice_id'] }
        @settings['voice_id'] = ids.first.to_s unless ids.include?(@settings['voice_id'])
        @message = voices.empty? ? 'Connected, but this key cannot see any voices.' : "Connected: #{voices.length} voices available."
      end

      def refresh_failed(message)
        @message = message
      end

      def synthesize(text:, output:, voice: nil, model: nil, tts: TTS.new)
        raise Error, 'Add your ElevenLabs API key first' unless configured?
        id = voice || @settings['voice_id']
        raise Error, 'Select an ElevenLabs voice first' if id.to_s.empty?
        raise Error, 'The selected voice is not in your ElevenLabs list. Refresh voices.' if !@voices.empty? && !@voices.any? { |v| v['voice_id'] == id }
        tts.synthesize(provider: 'elevenlabs', text: text, output: output, api_key: @api_key,
          voice: id, model: model || @settings['model'])
        @message = 'ElevenLabs connected; speech generated successfully.'
        output
      end

      def get_page(token)
        uri = URI('https://api.elevenlabs.io/v2/voices')
        uri.query = URI.encode_www_form({ page_size: 100 }.tap { |q| q[:next_page_token] = token if token })
        request = Net::HTTP::Get.new(uri, 'xi-api-key' => @api_key)
        response = Net::HTTP.start(uri.host, uri.port, use_ssl: true, open_timeout: 10, read_timeout: 30) { |http| http.request(request) }
        unless response.is_a?(Net::HTTPSuccess)
          message = { '401' => 'ElevenLabs rejected the API key.',
            '403' => 'This key does not have permission to list voices.',
            '429' => 'ElevenLabs rate limit reached.' }.fetch(response.code, 'ElevenLabs voice request failed.')
          raise Error, "#{message} (HTTP #{response.code})"
        end
        JSON.parse(response.body)
      rescue JSON::ParserError
        raise Error, 'ElevenLabs returned an invalid voice list'
      end
    end
  end
end
