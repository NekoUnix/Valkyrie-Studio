# frozen_string_literal: true
module Live2D
  module Tracking
    ARKIT = %w[browDownLeft browDownRight browInnerUp browOuterUpLeft browOuterUpRight
      cheekPuff cheekSquintLeft cheekSquintRight eyeBlinkLeft eyeBlinkRight eyeLookDownLeft
      eyeLookDownRight eyeLookInLeft eyeLookInRight eyeLookOutLeft eyeLookOutRight eyeLookUpLeft
      eyeLookUpRight eyeSquintLeft eyeSquintRight eyeWideLeft eyeWideRight jawForward jawLeft
      jawOpen jawRight mouthClose mouthDimpleLeft mouthDimpleRight mouthFrownLeft mouthFrownRight
      mouthFunnel mouthLeft mouthLowerDownLeft mouthLowerDownRight mouthPressLeft mouthPressRight
      mouthPucker mouthRight mouthRollLower mouthRollUpper mouthShrugLower mouthShrugUpper
      mouthSmileLeft mouthSmileRight mouthStretchLeft mouthStretchRight mouthUpperUpLeft
      mouthUpperUpRight noseSneerLeft noseSneerRight tongueOut].freeze
    ANGLES = %w[yaw pitch roll].freeze
    EXTENSIONS = %w[ParamMouthFunnel ParamMouthPress ParamMouthShrug ParamCheekPuff
      ParamTongueOut ParamMouthCornerRound ParamEyeSquint ParamBrowDepth].freeze
    KEYS = (ARKIT + ANGLES + EXTENSIONS).freeze

    # Accept the documented iFacialMocap pipe protocol and an explicit normalized
    # JSON dialect. UDP 8001 is a compatibility input, not VTS's public WS API.
    class Parser
      def self.parse(packet)
        raise Error, 'Packet too large' if packet.bytesize > 65_536
        if packet.lstrip.start_with?('{')
          obj = JSON.parse(packet)
          raw = obj.fetch('blendshapes', obj.fetch('parameters', obj))
          raw = raw.to_h { |v| [v.fetch('id', v['name']), v['value']] } if raw.is_a?(Array)
          raise Error, 'Expected parameter object' unless raw.is_a?(Hash)
          raw = raw.merge(obj.fetch('head', {}))
          clean(raw)
        else
          raw = {}
          packet.split('|').each do |field|
            if field.start_with?('=head#')
              angles = field.delete_prefix('=head#').split(',').first(3)
              raise Error, 'Invalid head rotation' unless angles.length == 3
              raw.merge!('pitch' => angles[0], 'yaw' => angles[1], 'roll' => angles[2])
            elsif (match = field.match(/\A([A-Za-z][A-Za-z0-9_]*)(?:-|&)(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)\z/))
              raw[match[1]] = Float(match[2]) / 100.0
            end
          end
          clean(raw)
        end
      rescue JSON::ParserError, KeyError, TypeError, ArgumentError => e
        raise Error, "Invalid tracking packet: #{e.message}"
      end

      def self.clean(raw)
        raw.each_with_object({}) do |(key, value), out|
          key = key.sub(/_L\z/, 'Left').sub(/_R\z/, 'Right')
          next unless KEYS.include?(key)
          out[key] = Live2D.number(value, min: ANGLES.include?(key) ? -180 : 0,
            max: ANGLES.include?(key) ? 180 : 1)
        end
      end
    end

    class Calibration
      attr_reader :count, :offsets
      def initialize(samples: 45)
        @target = samples
        reset
      end

      def reset
        @count, @samples, @offsets = 0, Hash.new { |h, k| h[k] = [] }, {}
      end

      def ready? = @count >= @target

      def apply(values)
        unless ready?
          values.each { |k, v| @samples[k] << v }
          @count += 1
          if ready?
            @offsets = @samples.transform_values { |v| v.sort[v.size / 2] }
          end
        end
        values.transform_keys(&:to_s).to_h do |k, v|
          baseline = @offsets.fetch(k, 0.0)
          adjusted = if ANGLES.include?(k)
            v - baseline
          else
            # Neutral offsets plus remaining expression range. Retains subtle blinks.
            [(v - baseline) / [1.0 - baseline, 0.15].max, 0.0].max.clamp(0, 1)
          end
          [k, adjusted]
        end
      end
    end

    class Mapper
      attr_reader :schema
      def initialize(schema)
        @schema = schema.to_h { |p| [p.fetch('id'), p] }
      end

      def clamp(id, value)
        p = @schema[id]
        p && value.clamp(p.fetch('min'), p.fetch('max'))
      end

      def defaults = @schema.transform_values { |p| p.fetch('default') }

      def map(s)
        get = ->(key) { s.fetch(key, 0.0) }
        avg = ->(a, b) { (get.call(a) + get.call(b)) / 2 }
        smile = avg.call('mouthSmileLeft', 'mouthSmileRight')
        frown = avg.call('mouthFrownLeft', 'mouthFrownRight')
        funnel = [get.call('mouthFunnel'), get.call('ParamMouthFunnel')].max
        pucker = [get.call('mouthPucker'), get.call('ParamMouthCornerRound')].max
        press = [avg.call('mouthPressLeft', 'mouthPressRight'), get.call('ParamMouthPress')].max
        shrug = [avg.call('mouthShrugLower', 'mouthShrugUpper'), get.call('ParamMouthShrug')].max
        puff = [get.call('cheekPuff'), get.call('ParamCheekPuff')].max
        tongue = [get.call('tongueOut'), get.call('ParamTongueOut')].max
        out = {
          'ParamAngleX' => get.call('yaw'), 'ParamAngleY' => get.call('pitch'), 'ParamAngleZ' => get.call('roll'),
          'ParamBodyAngleX' => get.call('yaw') * 0.3, 'ParamBodyAngleZ' => get.call('roll') * 0.25,
          'ParamEyeLOpen' => 1.0 - get.call('eyeBlinkLeft') + get.call('eyeWideLeft') * 0.3,
          'ParamEyeROpen' => 1.0 - get.call('eyeBlinkRight') + get.call('eyeWideRight') * 0.3,
          'ParamEyeLSmile' => get.call('cheekSquintLeft'), 'ParamEyeRSmile' => get.call('cheekSquintRight'),
          'ParamEyeBallX' => avg.call('eyeLookOutLeft', 'eyeLookInRight') - avg.call('eyeLookInLeft', 'eyeLookOutRight'),
          'ParamEyeBallY' => avg.call('eyeLookUpLeft', 'eyeLookUpRight') - avg.call('eyeLookDownLeft', 'eyeLookDownRight'),
          'ParamBrowLY' => get.call('browInnerUp') + get.call('browOuterUpLeft') - get.call('browDownLeft'),
          'ParamBrowRY' => get.call('browInnerUp') + get.call('browOuterUpRight') - get.call('browDownRight'),
          'ParamBrowLAngle' => get.call('browOuterUpLeft') - get.call('browDownLeft'),
          'ParamBrowRAngle' => get.call('browOuterUpRight') - get.call('browDownRight'),
          'ParamBrowLForm' => get.call('browInnerUp') * 0.3 - get.call('browDownLeft'),
          'ParamBrowRForm' => get.call('browInnerUp') * 0.3 - get.call('browDownRight'),
          'ParamMouthOpenY' => get.call('jawOpen') * (1 - get.call('mouthClose')),
          'ParamMouthForm' => smile - frown,
          'ParamMouthFunnel' => funnel, 'ParamMouthPress' => press, 'ParamMouthShrug' => shrug,
          'ParamCheekPuff' => puff, 'ParamTongueOut' => tongue,
          'ParamMouthCornerRound' => [pucker, get.call('ParamMouthCornerRound')].max,
          'ParamEyeSquint' => [avg.call('eyeSquintLeft', 'eyeSquintRight'), get.call('ParamEyeSquint')].max,
          'ParamBrowDepth' => [avg.call('browDownLeft', 'browDownRight'), get.call('ParamBrowDepth')].max
        }
        out['ParamMouthForm'] -= 0.7 * funnel unless @schema.key?('ParamMouthFunnel')
        out['ParamMouthForm'] -= 0.6 * pucker unless @schema.key?('ParamMouthCornerRound')
        out['ParamMouthOpenY'] *= 1 - press * 0.7 unless @schema.key?('ParamMouthPress')
        out['ParamMouthOpenY'] += shrug * 0.12 unless @schema.key?('ParamMouthShrug')
        out['ParamMouthOpenY'] += tongue * 0.2 unless @schema.key?('ParamTongueOut')
        out['ParamMouthForm'] += puff * 0.15 unless @schema.key?('ParamCheekPuff')
        unless @schema.key?('ParamEyeSquint')
          out['ParamEyeLOpen'] *= 1 - [get.call('eyeSquintLeft'), get.call('ParamEyeSquint')].max * 0.35
          out['ParamEyeROpen'] *= 1 - [get.call('eyeSquintRight'), get.call('ParamEyeSquint')].max * 0.35
        end
        unless @schema.key?('ParamBrowDepth')
          out['ParamBrowLY'] -= get.call('ParamBrowDepth') * 0.5
          out['ParamBrowRY'] -= get.call('ParamBrowDepth') * 0.5
        end
        # Common rig-specific ARKit-style names, discovered rather than assumed.
        ARKIT.each do |key|
          %W[Param#{key[0].upcase}#{key[1..]} #{key}].each { |id| out[id] = get.call(key) if @schema.key?(id) && !out.key?(id) }
        end
        out.each_with_object({}) { |(id, value), h| h[id] = clamp(id, value) if @schema.key?(id) }
      end
    end
  end
end
