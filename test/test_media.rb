# frozen_string_literal: true
require 'minitest/autorun'
require 'tmpdir'
require_relative '../lib/live2d_studio'

class MediaTest < Minitest::Test
  def setup
    @ffmpeg = ENV.fetch('FFMPEG', 'ffmpeg')
    @ffprobe = ENV.fetch('FFPROBE', 'ffprobe')
    skip 'Set MEDIA_TESTS=1 to run encoder integration tests' unless ENV['MEDIA_TESTS'] == '1'
    @dir = Dir.mktmpdir('valkyrie-media')
  end
  def teardown = FileUtils.remove_entry(@dir) if @dir && File.directory?(@dir)

  def test_alpha_codec_roundtrip_and_frame_count
    %w[vp9 prores].each do |codec|
      ext = Live2D::Exporter::CODECS[codec][0]
      output = File.join(@dir, "alpha#{ext}")
      exporter = Live2D::Exporter.new(output: output, width: 32, height: 32, fps: 30, codec: codec, ffmpeg: @ffmpeg)
      row = ([200, 60, 120, 0] * 8 + [200, 60, 120, 128] * 16 + [200, 60, 120, 255] * 8).pack('C*')
      6.times { exporter.push(row * 32) }
      exporter.finish
      probe, error, status = Open3.capture3(@ffprobe, '-v', 'error', '-count_frames', '-show_streams', '-of', 'json', output)
      assert status.success?, error
      assert_equal '6', JSON.parse(probe)['streams'][0]['nb_read_frames']
      command = [@ffmpeg, '-v', 'error']
      command += ['-c:v', 'libvpx-vp9'] if codec == 'vp9'
      raw, error, status = Open3.capture3(*command, '-i', output, '-frames:v', '1', '-f', 'rawvideo', '-pix_fmt', 'rgba', '-', binmode: true)
      assert status.success?, error
      assert_equal 32 * 32 * 4, raw.bytesize
      assert_operator raw.getbyte(3), :<=, 2
      assert_in_delta 128, raw.getbyte(12 * 4 + 3), 3
      assert_operator raw.getbyte(31 * 4 + 3), :>=, 253
    end
  end
  def test_background_mp4_codecs_and_audio_mux
    wav = File.join(@dir, 'tone.wav')
    _, error, status = Open3.capture3(@ffmpeg, '-v', 'error', '-f', 'lavfi', '-i', 'sine=frequency=440:duration=0.2', wav)
    assert status.success?, error
    %w[h264 h265].each do |codec|
      output = File.join(@dir, "#{codec}.mp4")
      exporter = Live2D::Exporter.new(output: output, width: 32, height: 32, fps: 30, codec: codec, audio: wav, ffmpeg: @ffmpeg)
      6.times { exporter.push([100, 120, 180, 255].pack('C*') * 32 * 32) }
      exporter.finish
      probe, error, status = Open3.capture3(@ffprobe, '-v', 'error', '-show_streams', '-of', 'json', output)
      assert status.success?, error
      assert_equal %w[video audio], JSON.parse(probe)['streams'].map { |s| s['codec_type'] }
    end
  end
  def test_audio_decode_envelope_and_visemes
    wav = File.join(@dir, 'voice.wav')
    _, error, result = Open3.capture3(@ffmpeg, '-v', 'error', '-f', 'lavfi', '-i', 'sine=frequency=400:duration=0.3', wav)
    assert result.success?, error
    cue_path = File.join(@dir, 'cues.json')
    File.write(cue_path, JSON.generate(mouthCues: [{ start: 0, end: 0.15, value: 'D' }, { start: 0.15, end: 0.3, value: 'F' }]))
    clip = Live2D::Audio::Clip.new(wav, ffmpeg: @ffmpeg, cues_path: cue_path)
    assert_in_delta 0.3, clip.duration, 0.001
    assert_operator clip.mouth(0.1)['ParamMouthOpenY'], :>, clip.mouth(0.25)['ParamMouthOpenY']
    assert_operator clip.mouth(0.25)['ParamMouthForm'], :<, 0
    assert_equal 0, clip.mouth(1)['ParamMouthOpenY']
  ensure
    clip&.close
  end
  def test_audio_is_trimmed_to_exact_video_duration
    wav = File.join(@dir, 'long.wav')
    _, error, code = Open3.capture3(@ffmpeg, '-v', 'error', '-f', 'lavfi', '-i', 'sine=duration=1', wav)
    assert code.success?, error
    output = File.join(@dir, 'trimmed.mov')
    exporter = Live2D::Exporter.new(output: output, width: 32, height: 32, fps: 30, codec: 'prores', audio: wav, ffmpeg: @ffmpeg)
    6.times { exporter.push([100, 120, 180, 128].pack('C*') * 32 * 32) }
    exporter.finish
    result, = Open3.capture3(@ffprobe, '-v', 'error', '-show_streams', '-of', 'json', output)
    streams = JSON.parse(result)['streams']
    streams.each { |stream| assert_in_delta 0.2, Float(stream['duration']), 0.001 }
  end

  def test_live_queue_never_waits_and_preserves_duration_and_alpha
    output = File.join(@dir, 'live.webm')
    exporter = Live2D::Exporter.new(output: output, width: 32, height: 32, fps: 30,
      codec: 'vp9', realtime: true, queue_frames: 1, ffmpeg: @ffmpeg)
    # Gate the pipe to simulate a stalled encoder without relying on CPU speed.
    input = exporter.instance_variable_get(:@stdin)
    original_write = input.method(:write)
    entered, release = Queue.new, Queue.new
    input.define_singleton_method(:write) do |bytes|
      entered << true
      release.pop
      original_write.call(bytes)
    end
    pixels = [80, 140, 200, 128].pack('C*') * 32 * 32
    assert exporter.push(pixels, frame: 0)
    Timeout.timeout(5) { entered.pop }
    assert exporter.push(pixels, frame: 1)
    start = Live2D.monotonic
    refute exporter.push(pixels, frame: 2)
    assert_operator Live2D.monotonic - start, :<, 0.1
    assert_equal 1, exporter.dropped
    10.times { release << true }
    exporter.finish(total_frames: 6)
    assert_equal 6, exporter.frames
    assert_equal 4, exporter.duplicates
    assert_equal 'saved', exporter.state
    refute File.exist?(exporter.intermediate)
    probe, = Open3.capture3(@ffprobe, '-v', 'error', '-count_frames', '-show_streams', '-of', 'json', output)
    assert_equal '6', JSON.parse(probe)['streams'][0]['nb_read_frames']
    raw, err, status = Open3.capture3(@ffmpeg, '-v', 'error', '-c:v', 'libvpx-vp9', '-i', output,
      '-frames:v', '1', '-f', 'rawvideo', '-pix_fmt', 'rgba', '-', binmode: true)
    assert status.success?, err
    assert_in_delta 128, raw.getbyte(3), 3
  ensure
    10.times { release << true } if release
    exporter&.abort unless exporter&.state == 'saved'
  end
end
