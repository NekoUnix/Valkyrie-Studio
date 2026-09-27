# frozen_string_literal: true
module Live2D
  # Composition estimates, not platform guarantees. Margins are fractions of the
  # portrait canvas, ordered left/top/right/bottom. Keep them editable because UI
  # placement varies by device, captions, organic/ad placement and app revision.
  module Guides
    PRESETS = {
      'TikTok' => [0.06, 0.10, 0.20, 0.25],
      'YouTube Shorts' => [0.06, 0.10, 0.20, 0.23],
      'Instagram Reels' => [0.06, 0.14, 0.18, 0.35],
      'Facebook Reels' => [0.06, 0.14, 0.18, 0.35],
      'Instagram / Facebook Stories' => [0.06, 0.14, 0.06, 0.20],
      'Snapchat Spotlight' => [0.06, 0.12, 0.18, 0.23],
      'Snapchat Stories' => [0.06, 0.12, 0.06, 0.18],
      'Pinterest' => [0.06, 0.10, 0.12, 0.22],
      'X video' => [0.06, 0.10, 0.08, 0.22],
      'LinkedIn video' => [0.06, 0.12, 0.08, 0.24],
      'Threads / Bluesky video' => [0.06, 0.10, 0.08, 0.22],
      'Twitch vertical clips' => [0.06, 0.12, 0.18, 0.24],
      'WhatsApp Status' => [0.06, 0.12, 0.06, 0.20]
    }.freeze
    DEFAULT = { 'preset' => 'All platforms', 'enabled' => true, 'mock_ui' => true, 'opacity' => 0.30 }.freeze
    def self.validate(settings)
      result = DEFAULT.merge(settings)
      raise Error, 'Unknown social guide' unless ['All platforms', 'Custom', *PRESETS.keys].include?(result['preset'])
      result['opacity'] = Live2D.number(result['opacity'], min: 0, max: 0.8)
      if result['preset'] == 'Custom'
        m = result.fetch('margins')
        raise Error, 'Four guide margins required' unless m.is_a?(Array) && m.size == 4
        m = m.map { |v| Live2D.number(v, min: 0, max: 0.45) }
        result['margins'] = m
      end
      result
    end
    def self.state(settings)
      name = settings['preset']
      margins = if name == 'All platforms'
        PRESETS.values.transpose.map(&:max)
      elsif name == 'Custom'
        settings['margins']
      else
        PRESETS.fetch(name)
      end
      settings.merge('margins' => margins, 'presets' => ['All platforms', *PRESETS.keys, 'Custom'])
    end
  end
end
