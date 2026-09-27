# frozen_string_literal: true
require 'minitest/autorun'
require 'tmpdir'
require_relative '../lib/live2d_studio'
require_relative '../lib/live2d_studio/performance'
require_relative '../lib/live2d_studio/performance_runner'

class PerformanceTest < Minitest::Test
  def test_extended_view_range_accepts_new_boundaries
    plan = Live2D::Performance.new('view' => { 'zoom' => 30, 'x' => 12, 'y' => -12 },
                                   'lines' => [{ 'text' => 'Framing test' }])
    assert_equal({ 'zoom' => 30.0, 'x' => 12.0, 'y' => -12.0 }, plan.view)
    assert_raises(Live2D::Error) do
      Live2D::Performance.new('view' => { 'zoom' => 30.01 },
                              'lines' => [{ 'text' => 'Framing test' }])
    end
    assert_raises(Live2D::Error) do
      Live2D::Performance.new('view' => { 'x' => 12.01 },
                              'lines' => [{ 'text' => 'Framing test' }])
    end
  end

  def test_measured_voice_lengths_drive_chapter_boundaries_and_motion
    plan = Live2D::Performance.new('chapter_seconds' => 10, 'lead_in' => 0.5, 'tail' => 1,
      'lines' => [
        { 'text' => 'First line', 'pause' => 0.5, 'motion' => [
          { 'at' => 0, 'yaw' => -12 }, { 'at' => 1, 'yaw' => 12 }] },
        { 'text' => 'Second line', 'pause' => 0.5 },
        { 'text' => 'Third line', 'pause' => 0.5 }
      ])
    chapters = plan.chapters([3, 3, 3].map { |seconds| seconds * Live2D::Performance::RATE })
    assert_equal 2, chapters.size
    assert_equal [0, 1], chapters[0][:entries].map { |entry| entry[:index] }
    assert_equal [2], chapters[1][:entries].map { |entry| entry[:index] }
    assert_in_delta 7.5, chapters[0][:samples].to_f / Live2D::Performance::RATE
    assert_equal 0, chapters[0][:tail_samples]
    assert_equal 0, chapters[1][:lead_samples]
    assert_equal chapters[0][:samples], chapters[1][:global_start_samples]
    assert_in_delta 0, plan.tracking(chapters[0], 2.0)['yaw'], 0.01
    assert_in_delta 12, plan.tracking(chapters[0], 3.49)['yaw'], 0.1
    refute plan.tracking(chapters[0], 2.0).key?('jawOpen')
    timeline = plan.timeline(chapters[0])
    assert_equal 7.5, timeline['duration']
    assert_equal 'background', timeline['events'][1].dig('command', 'op')
    assert timeline['events'].any? { |event| event.dig('command', 'op') == 'tracking' }
  end

  def test_invalid_motion_and_oversized_line_rejected
    assert_raises(Live2D::Error) do
      Live2D::Performance.new('lines' => [{ 'text' => 'Bad cue', 'motion' => [
        { 'at' => 0.8, 'yaw' => 4 }, { 'at' => 0.2, 'yaw' => -4 }] }])
    end
    plan = Live2D::Performance.new('chapter_seconds' => 10, 'lines' => [{ 'text' => 'Too long' }])
    assert_raises(Live2D::Error) { plan.chapters([11 * Live2D::Performance::RATE]) }
  end

  def test_silent_padding_is_trimmed_without_removing_mid_line_pause
    Dir.mktmpdir('performance-audio') do |dir|
      raw = File.join(dir, 'raw.pcm')
      trimmed = File.join(dir, 'trimmed.pcm')
      tone = Array.new(4800) { |i| (Math.sin(i * 0.04) * 9000).round }
      pause = Array.new(4800, 0)
      File.binwrite(raw, (Array.new(14_400, 0) + tone + pause + tone + Array.new(19_200, 0)).pack('s<*'))
      plan = Live2D::Performance.new('lines' => [{ 'text' => 'Test' }])
      runner = Live2D::PerformanceRunner.new(plan, script_path: raw, output: File.join(dir, 'new.mp4'))
      runner.send(:trim_silent_edges, raw, trimmed)
      result = File.binread(trimmed).unpack('s<*')
      assert_operator result.length, :<, File.size(raw) / 2
      assert_operator result.length, :>, 14_400
      assert_includes File.binread(trimmed), "\0" * 9600
    end
  end
end
