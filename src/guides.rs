//! Editable portrait composition estimates. Platform UI can change, so these
//! guides are preview overlays and never enter the render target.
pub const PRESETS: &[(&str, [f32; 4])] = &[
    ("TikTok", [0.06, 0.10, 0.20, 0.25]),
    ("YouTube Shorts", [0.06, 0.10, 0.20, 0.23]),
    ("Instagram Reels", [0.06, 0.14, 0.18, 0.35]),
    ("Facebook Reels", [0.06, 0.14, 0.18, 0.35]),
    ("Instagram / Facebook Stories", [0.06, 0.14, 0.06, 0.20]),
    ("Snapchat Spotlight", [0.06, 0.12, 0.18, 0.23]),
    ("Snapchat Stories", [0.06, 0.12, 0.06, 0.18]),
    ("Pinterest", [0.06, 0.10, 0.12, 0.22]),
    ("X video", [0.06, 0.10, 0.08, 0.22]),
    ("LinkedIn video", [0.06, 0.12, 0.08, 0.24]),
    ("Threads / Bluesky video", [0.06, 0.10, 0.08, 0.22]),
    ("Twitch vertical clips", [0.06, 0.12, 0.18, 0.24]),
    ("WhatsApp Status", [0.06, 0.12, 0.06, 0.20]),
];

pub fn margins(preset: &str, custom: [f32; 4]) -> Option<[f32; 4]> {
    match preset {
        "Custom" => Some(custom),
        "All platforms" => Some(PRESETS.iter().fold([0.0; 4], |mut acc, (_, margin)| {
            for i in 0..4 {
                acc[i] = acc[i].max(margin[i]);
            }
            acc
        })),
        _ => PRESETS
            .iter()
            .find(|(name, _)| *name == preset)
            .map(|(_, margin)| *margin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combined_safe_area_contains_every_platform() {
        let all = margins("All platforms", [0.0; 4]).unwrap();
        for (_, values) in PRESETS {
            for i in 0..4 {
                assert!(all[i] >= values[i]);
            }
        }
    }
}
