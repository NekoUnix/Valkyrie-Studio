require 'minitest/autorun'
require 'tmpdir'
require_relative '../lib/live2d_studio'

class VoiceTest < Minitest::Test
  class MemoryStore
    attr_accessor :value
    def persistent? = true
    def read = value
    def write(key) = self.value = key
    def delete = self.value = nil
  end
  class CaptureTTS < Live2D::Audio::TTS
    attr_reader :request
    def post(url, headers, body, output)
      @request = { url: url, headers: headers, body: body, output: output }
    end
  end

  def setup
    @old_key = ENV.delete('OPENAI_API_KEY')
  end
  def teardown
    ENV['OPENAI_API_KEY'] = @old_key if @old_key
  end

  def test_voice_key_is_not_in_status_and_speech_uses_selected_controls
    store = MemoryStore.new
    voice = Live2D::Audio::Voice.new({}, store: store)
    refute voice.configured?
    assert_raises(Live2D::Error) { voice.synthesize(text: 'Hello', output: 'unused.wav') }
    fake = 'sk-test-only-not-a-real-api-key'
    voice.set_key(fake)
    assert_equal fake, store.value
    voice.configure('voice' => 'marin', 'instructions' => 'Speak gently.', 'speed' => 0.85)
    tts = CaptureTTS.new
    voice.synthesize(text: 'Hello', output: 'unused.wav', tts: tts)
    assert_equal 'https://api.openai.com/v1/audio/speech', tts.request[:url]
    assert_equal "Bearer #{fake}", tts.request[:headers]['Authorization']
    assert_equal 'marin', tts.request[:body][:voice]
    assert_equal 'Speak gently.', tts.request[:body][:instructions]
    assert_equal 0.85, tts.request[:body][:speed]
    assert_equal 'wav', tts.request[:body][:response_format]
    refute_includes JSON.generate(voice.status), fake
    refute_includes JSON.generate(voice.settings), fake
    voice.forget_key
    refute voice.configured?
    assert_nil store.value
  end

  def test_validation_and_legacy_model_options
    voice = Live2D::Audio::Voice.new({}, store: MemoryStore.new)
    assert_raises(Live2D::Error) { voice.configure('voice' => 'made-up') }
    assert_raises(Live2D::Error) { voice.configure('speed' => Float::NAN) }
    assert_raises(Live2D::Error) { voice.configure('model' => 'tts-1', 'voice' => 'marin') }
    tts = CaptureTTS.new
    tts.synthesize(provider: 'openai', text: 'Hello', output: 'unused', api_key: 'fake', model: 'tts-1', instructions: 'Ignored')
    refute tts.request[:body].key?(:instructions)
  end

  def test_session_key_removes_previous_saved_key
    store = MemoryStore.new
    store.value = 'sk-previous-test-only-key'
    voice = Live2D::Audio::Voice.new({}, store: store)
    voice.set_key('sk-session-test-only-key', remember: false)
    assert_nil store.value
    assert voice.configured?
  end

  def test_windows_key_roundtrip_is_encrypted
    skip 'Windows DPAPI only' unless Gem.win_platform?
    Dir.mktmpdir('voice-key-test') do |dir|
      path = File.join(dir, 'key.dpapi')
      store = Live2D::Audio::KeyStore.new(path)
      fake = 'sk-test-only-not-a-real-api-key'
      store.write(fake)
      refute_includes File.binread(path), fake
      assert_equal fake, store.read
      store.delete
      refute File.exist?(path)
    end
  end
end
