require 'minitest/autorun'
require_relative '../lib/live2d_studio'

class ElevenLabsTest < Minitest::Test
  class MemoryStore
    attr_accessor :value
    def persistent? = true
    def read = value
    def write(key) = self.value = key
    def delete = self.value = nil
  end

  class FakeElevenLabs < Live2D::Audio::ElevenLabs
    attr_accessor :pages
    def get_page(token) = pages.fetch(token)
  end

  class CaptureTTS < Live2D::Audio::TTS
    attr_reader :request
    def post(url, headers, body, output)
      @request = { url: url, headers: headers, body: body, output: output }
    end
  end

  def setup
    @old_key = ENV.delete('ELEVENLABS_API_KEY')
  end

  def teardown
    ENV['ELEVENLABS_API_KEY'] = @old_key if @old_key
  end

  def test_account_voices_paginate_and_selected_voice_generates_speech_without_exposing_key
    store = MemoryStore.new
    eleven = FakeElevenLabs.new({}, store: store)
    key = 'test-only-elevenlabs-secret'
    eleven.set_key(key)
    eleven.pages = {
      nil => { 'voices' => [{ 'voice_id' => 'z-id', 'name' => 'Zoe' }], 'has_more' => true, 'next_page_token' => 'next' },
      'next' => { 'voices' => [{ 'voice_id' => 'a-id', 'name' => 'Alice' },
        { 'voice_id' => 'z-id', 'name' => 'Zoe' }], 'has_more' => false }
    }
    eleven.apply_voices(eleven.fetch_voices)
    assert_equal %w[Alice Zoe], eleven.status['voices'].map { |v| v['name'] }
    assert_equal 'a-id', eleven.settings['voice_id']
    eleven.configure('voice_id' => 'z-id')
    tts = CaptureTTS.new
    eleven.synthesize(text: 'Hello', output: 'unused.mp3', tts: tts)
    assert_includes tts.request[:url], '/z-id?output_format=mp3_44100_128'
    assert_equal key, tts.request[:headers]['xi-api-key']
    assert_equal 'eleven_multilingual_v2', tts.request[:body][:model_id]
    refute_includes JSON.generate(eleven.status), key
    refute_includes JSON.generate(eleven.settings), key
    assert_equal key, store.value
    eleven.forget_key
    assert_nil store.value
    refute eleven.configured?
  end

  def test_missing_or_unlisted_voice_does_not_call_provider
    eleven = FakeElevenLabs.new({}, store: MemoryStore.new)
    eleven.set_key('test-only-elevenlabs-secret', remember: false)
    assert_raises(Live2D::Error) { eleven.synthesize(text: 'Hello', output: 'unused.mp3') }
    eleven.apply_voices([{ 'voice_id' => 'known', 'name' => 'Known' }])
    assert_raises(Live2D::Error) { eleven.synthesize(text: 'Hello', output: 'unused.mp3', voice: 'unknown') }
  end
end
