# frozen_string_literal: true
module Live2D
  class Engine
    EMOTIONS = {
      'joy' => { 'mouthSmileLeft' => 0.85, 'mouthSmileRight' => 0.85, 'cheekSquintLeft' => 0.4, 'cheekSquintRight' => 0.4 },
      'thinking' => { 'yaw' => -10, 'roll' => 7, 'browOuterUpRight' => 0.5 },
      'angry' => { 'browDownLeft' => 0.8, 'browDownRight' => 0.8, 'mouthFrownLeft' => 0.6, 'mouthFrownRight' => 0.6 },
      'surprised' => { 'jawOpen' => 0.7, 'eyeWideLeft' => 0.8, 'eyeWideRight' => 0.8, 'browInnerUp' => 0.8 },
      'neutral' => {}
    }.freeze
    attr_reader :mode, :calibration

    def initialize(config)
      @config = config
      @mode = config.fetch('mode', 'agent')
      @calibration = Tracking::Calibration.new(samples: config.fetch('calibration_samples', 45))
      @inputs, @direct, @emotion, @filtered = {}, {}, {}, {}
    end

    def mode=(mode)
      raise Error, 'Mode must be agent, phone, webcam or idle' unless %w[agent phone webcam idle].include?(mode)
      @mode = mode
      @calibration.reset
      @filtered.clear
    end

    def ingest(source, values, time)
      raise Error, 'Unknown tracking source' unless %w[agent phone webcam].include?(source)
      clean = Tracking::Parser.clean(values)
      clean = @calibration.apply(clean) if source == @mode && %w[phone webcam].include?(source)
      @inputs[source] = [clean, time]
    end

    def emotion(name, intensity, until_time)
      preset = EMOTIONS.fetch(name) { raise Error, 'Unknown emotion' }
      intensity = Live2D.number(intensity, min: 0, max: 1)
      @emotion = { values: preset.transform_values { |v| v * intensity }, until: until_time }
    end

    def parameters(values, until_time)
      raise Error, 'parameters must be an object with at most 512 IDs' unless values.is_a?(Hash) && values.size <= 512
      clean = values.to_h { |k, v| [k.to_s, [Live2D.number(v), until_time]] }
      @direct.merge!(clean)
    end

    def clear_parameters = @direct.clear

    def sample(mapper, time, dt, mouth: nil, key: 'primary')
      values, stamp = @inputs.fetch(@mode, [{}, -Float::INFINITY])
      values = {} if time - stamp > @config.fetch('stale_after', 0.5)
      values = values.merge(@emotion[:values]) if @emotion[:until] && time < @emotion[:until]
      desired = mapper.defaults.merge(mapper.map(values))
      desired['ParamBreath'] = (Math.sin(time * 1.8) + 1) * 0.5 if mapper.schema.key?('ParamBreath')
      if values.empty?
        desired['ParamAngleZ'] = Math.sin(time * 0.7) * 1.2 if mapper.schema.key?('ParamAngleZ')
        phase = time % 4.7
        blink = phase < 0.15 ? (phase / 0.075 - 1).abs : 1.0
        %w[ParamEyeLOpen ParamEyeROpen].each { |id| desired[id] = blink if mapper.schema.key?(id) }
      end
      desired.merge!(mouth.select { |id, _| mapper.schema.key?(id) }) if mouth
      @direct.delete_if { |_, (_, expiry)| time >= expiry }
      @direct.each { |id, (v, _)| desired[id] = v if mapper.schema.key?(id) }
      previous = (@filtered[key] ||= mapper.defaults)
      alpha = 1 - Math.exp(-dt / @config.fetch('smoothing_seconds', 0.045).clamp(0.001, 1))
      desired.each { |id, v| previous[id] += (mapper.clamp(id, v) - previous.fetch(id)) * alpha }
      previous.dup
    end
  end

  class Timeline
    attr_reader :duration
    def initialize(path)
      data = JSON.parse(File.read(path))
      @duration = Live2D.number(data.fetch('duration'), min: 0.01, max: 86_400)
      @events = data.fetch('events').each_with_index.map do |event, index|
        time = Live2D.number(event.fetch('time'), min: 0, max: @duration)
        raise Error, 'Timeline command missing' unless event['command'].is_a?(Hash)
        [time, index, event['command']]
      end.sort_by { |time, index, _| [time, index] }
      @cursor = 0
    end

    def due(time)
      commands = []
      while @cursor < @events.size && @events[@cursor][0] <= time + 1e-9
        commands << @events[@cursor][2]
        @cursor += 1
      end
      commands
    end
  end
end
