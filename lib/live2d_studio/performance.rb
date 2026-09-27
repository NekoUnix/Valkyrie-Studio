# frozen_string_literal: true
require 'rbconfig'

module Live2D
  # An agent supplies lines and motion cues; the measured speech samples become
  # the clock for both offline lip-sync and head movement.
  class Performance
    RATE = 48_000
    AXES = %w[yaw pitch roll].freeze
    EMOTIONS = Engine::EMOTIONS.keys.freeze
    attr_reader :lines, :provider, :width, :height, :fps, :background, :view,
                :chapter_seconds, :lead_in, :tail, :model_path

    def initialize(data)
      raise Error, 'Performance must be a JSON object' unless data.is_a?(Hash)
      @provider = data.fetch('provider', 'elevenlabs')
      raise Error, 'Provider must be elevenlabs, openai or system' unless %w[elevenlabs openai system].include?(@provider)
      @model_path = data['model_path']
      raise Error, 'model_path must be text' if @model_path && !@model_path.is_a?(String)
      @width = Integer(data.fetch('width', 1920))
      @height = Integer(data.fetch('height', 1080))
      raise Error, 'Canvas dimensions must be even and 16–8192' unless [@width, @height].all? { |n| n.even? && n.between?(16, 8192) }
      @fps = Integer(data.fetch('fps', 30))
      raise Error, 'FPS must be 24, 30 or 60' unless [24, 30, 60].include?(@fps)
      @background = data.fetch('background', [0.04, 0.05, 0.08, 1.0])
      raise Error, 'background must be opaque RGBA' unless @background.is_a?(Array) && @background.length == 4 &&
        @background.map { |n| Live2D.number(n, min: 0, max: 1) }.last == 1.0
      @view = { 'zoom' => 1.0, 'x' => 0.0, 'y' => 0.0 }.merge(data.fetch('view', {}))
      raise Error, 'view must contain zoom, x and y' unless @view.is_a?(Hash)
      @view['zoom'] = Live2D.number(@view['zoom'], min: 0.1, max: 30)
      %w[x y].each { |axis| @view[axis] = Live2D.number(@view[axis], min: -12, max: 12) }
      @chapter_seconds = Live2D.number(data.fetch('chapter_seconds', 180), min: 10, max: 540)
      @lead_in = Live2D.number(data.fetch('lead_in', 0.2), min: 0, max: 10)
      @tail = Live2D.number(data.fetch('tail', 0.8), min: 0.2, max: 10)
      raw_lines = data.fetch('lines')
      raise Error, 'Provide 1–1000 voice lines' unless raw_lines.is_a?(Array) && raw_lines.length.between?(1, 1000)
      @lines = raw_lines.each_with_index.map { |line, index| validate_line(line, index) }
    rescue ArgumentError, TypeError => e
      raise Error, "Invalid performance: #{e.message}"
    end

    def validate_line(line, index)
      raise Error, "Line #{index + 1} must be an object" unless line.is_a?(Hash)
      text = line.fetch('text')
      raise Error, "Line #{index + 1} needs 1–4000 characters" unless text.is_a?(String) && text.strip.length.between?(1, 4000)
      pause = Live2D.number(line.fetch('pause', 0.18), min: 0, max: 30)
      emotion = line['emotion']
      raise Error, "Unknown emotion on line #{index + 1}" if emotion && !EMOTIONS.include?(emotion)
      voice = line['voice']
      raise Error, "Invalid voice on line #{index + 1}" if voice && (!voice.is_a?(String) || voice.empty? || voice.length > 100)
      tts_model = line['tts_model']
      raise Error, "Invalid speech model on line #{index + 1}" if tts_model &&
        (!tts_model.is_a?(String) || !tts_model.match?(/\A[A-Za-z0-9_-]{1,80}\z/))
      cues = line['motion']
      if cues
        raise Error, "Line #{index + 1} motion needs 1–50 cues" unless cues.is_a?(Array) && cues.length.between?(1, 50)
        cues = cues.map do |cue|
          raise Error, 'Motion cue must be an object' unless cue.is_a?(Hash)
          point = { 'at' => Live2D.number(cue.fetch('at'), min: 0, max: 1) }
          AXES.each { |axis| point[axis] = Live2D.number(cue.fetch(axis, 0), min: -45, max: 45) }
          point
        end
        raise Error, "Line #{index + 1} motion cue times must increase" unless cues.each_cons(2).all? { |a, b| a['at'] < b['at'] }
      end
      { 'text' => text, 'pause' => pause, 'emotion' => emotion, 'voice' => voice,
        'tts_model' => tts_model, 'motion' => cues }
    end

    def chapters(sample_counts)
      raise Error, 'Every line needs a measured audio duration' unless sample_counts.is_a?(Array) && sample_counts.length == @lines.length
      lead = (@lead_in * RATE).round
      tail_samples = (@tail * RATE).round
      max_samples = (@chapter_seconds * RATE).floor
      result = []
      chapter = { entries: [], samples: lead, lead_samples: lead, tail_samples: 0, global_start_samples: 0 }
      elapsed_samples = 0
      sample_counts.each_with_index do |count, index|
        raise Error, "Line #{index + 1} has no audio" unless count.is_a?(Integer) && count.positive?
        pause_samples = (@lines[index]['pause'] * RATE).round
        required = count + pause_samples
        raise Error, "Line #{index + 1} exceeds a chapter; split that line" if lead + required + tail_samples > max_samples
        if chapter[:samples] + required + tail_samples > max_samples && !chapter[:entries].empty?
          result << chapter
          elapsed_samples += chapter[:samples]
          chapter = { entries: [], samples: 0, lead_samples: 0, tail_samples: 0,
                      global_start_samples: elapsed_samples }
        end
        start_sample = chapter[:samples]
        chapter[:entries] << { index: index, line: @lines[index], start_sample: start_sample,
                               speech_samples: count, pause_samples: pause_samples }
        chapter[:samples] += required
      end
      chapter[:tail_samples] = tail_samples
      chapter[:samples] += tail_samples
      result << chapter
      result
    end

    def timeline(chapter)
      events = [
        { 'time' => 0, 'command' => { 'op' => 'mode', 'mode' => 'agent' } },
        { 'time' => 0, 'command' => { 'op' => 'background', 'color' => @background } },
        { 'time' => 0, 'command' => { 'op' => 'view', **@view } }
      ]
      chapter[:entries].each do |entry|
        next unless entry[:line]['emotion']
        events << { 'time' => entry[:start_sample].to_f / RATE,
                    'command' => { 'op' => 'emotion', 'name' => entry[:line]['emotion'],
                                   'duration' => entry[:speech_samples].to_f / RATE } }
      end
      frames = (chapter[:samples].to_f / RATE * @fps).ceil
      frames.times do |frame|
        time = frame.to_f / @fps
        events << { 'time' => time, 'command' => { 'op' => 'tracking', 'values' => tracking(chapter, time) } }
      end
      { 'duration' => chapter[:samples].to_f / RATE, 'events' => events }
    end

    def tracking(chapter, time)
      sample = (time * RATE).round
      entry = chapter[:entries].find do |item|
        sample >= item[:start_sample] && sample < item[:start_sample] + item[:speech_samples] + item[:pause_samples]
      end
      pose = if entry && sample < entry[:start_sample] + entry[:speech_samples]
        fraction = (sample - entry[:start_sample]).to_f / entry[:speech_samples]
        pose_for(entry, fraction, time + chapter[:global_start_samples].to_f / RATE)
      elsif entry && entry[:pause_samples].positive?
        progress = (sample - entry[:start_sample] - entry[:speech_samples]).to_f / entry[:pause_samples]
        pose_for(entry, 1.0, time + chapter[:global_start_samples].to_f / RATE).transform_values { |angle| angle * (1 - progress) }
      else
        AXES.to_h { |axis| [axis, 0.0] }
      end
      blink_phase = (time + chapter[:global_start_samples].to_f / RATE) % 3.4
      blink = blink_phase < 0.14 ? (1 - (blink_phase - 0.07).abs / 0.07).clamp(0, 1) : 0.0
      pose.merge('eyeBlinkLeft' => blink, 'eyeBlinkRight' => blink)
    end

    def pose_for(entry, fraction, time)
      cues = entry[:line]['motion']
      unless cues
        phase = time * 1.15 + entry[:index] * 0.4
        return { 'yaw' => Math.sin(phase) * 9, 'pitch' => Math.sin(phase * 0.63) * 3.5,
                 'roll' => Math.sin(phase * 0.77) * 2.5 }
      end
      return cues.first.slice(*AXES) if fraction <= cues.first['at']
      return cues.last.slice(*AXES) if fraction >= cues.last['at']
      left, right = cues.each_cons(2).find { |a, b| fraction.between?(a['at'], b['at']) }
      mix = (fraction - left['at']) / (right['at'] - left['at'])
      mix = mix * mix * (3 - 2 * mix)
      AXES.to_h { |axis| [axis, left[axis] + (right[axis] - left[axis]) * mix] }
    end
  end
end
