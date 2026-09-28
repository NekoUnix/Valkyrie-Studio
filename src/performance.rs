//! Script validation and sample-timed head movement for narrated performances.
use anyhow::{Result, bail, ensure};
use serde::Deserialize;
use std::path::PathBuf;

fn provider_default() -> String {
    "elevenlabs".into()
}
fn width_default() -> u32 {
    1080
}
fn height_default() -> u32 {
    1920
}
fn fps_default() -> u32 {
    30
}
fn chapter_default() -> f64 {
    180.0
}
fn pause_default() -> f64 {
    0.18
}
fn motion_energy_default() -> f32 {
    1.35
}

#[derive(Debug, Clone, Deserialize)]
pub struct Script {
    pub model_path: Option<PathBuf>,
    #[serde(default = "provider_default")]
    pub provider: String,
    /// Default speech model for all lines; a line can override it.
    pub tts_model: Option<String>,
    #[serde(default = "width_default")]
    pub width: u32,
    #[serde(default = "height_default")]
    pub height: u32,
    #[serde(default = "fps_default")]
    pub fps: u32,
    #[serde(default = "chapter_default")]
    pub chapter_seconds: f64,
    #[serde(default)]
    pub lead_in: f64,
    #[serde(default)]
    pub tail: f64,
    #[serde(default)]
    pub view: View,
    #[serde(default = "motion_energy_default")]
    pub motion_energy: f32,
    pub lines: Vec<Line>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct View {
    #[serde(default = "zoom_default")]
    pub zoom: f32,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}
fn zoom_default() -> f32 {
    1.0
}
impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            x: 0.0,
            y: 0.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Line {
    pub text: String,
    pub voice: Option<String>,
    pub tts_model: Option<String>,
    pub emotion: Option<String>,
    #[serde(default = "expression_default")]
    pub expression_intensity: f32,
    pub gesture: Option<String>,
    #[serde(default = "pause_default")]
    pub pause: f64,
    #[serde(default)]
    pub motion: Vec<Cue>,
    /// An existing local audio file can be supplied instead of spending TTS credits.
    pub audio: Option<PathBuf>,
}
fn expression_default() -> f32 {
    1.0
}

pub const EMOTIONS: &[&str] = &[
    "neutral",
    "joy",
    "excited",
    "curious",
    "thinking",
    "sad",
    "angry",
    "surprised",
];

pub fn v3_emotion_tag(emotion: &str) -> Option<&'static str> {
    match emotion {
        "joy" => Some("[happy]"),
        "excited" => Some("[excited]"),
        "curious" | "thinking" => Some("[curious]"),
        "sad" => Some("[sad]"),
        "angry" => Some("[angry]"),
        "surprised" => Some("[surprised]"),
        _ => None,
    }
}
pub fn default_gesture(emotion: &str) -> Option<&'static str> {
    match emotion {
        "joy" | "excited" => Some("nod"),
        "curious" | "thinking" | "sad" => Some("tilt"),
        "angry" => Some("shake"),
        "surprised" => Some("lean"),
        _ => None,
    }
}
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Cue {
    pub at: f64,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    #[serde(default)]
    pub roll: f32,
}
impl Script {
    pub fn parse(json: &str) -> Result<Self> {
        let script: Self = serde_json::from_str(json)?;
        script.validate()?;
        Ok(script)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.provider.as_str(), "elevenlabs" | "openai"),
            "Provider must be elevenlabs or openai"
        );
        ensure!(
            (16..=8192).contains(&self.width)
                && self.width % 2 == 0
                && (16..=8192).contains(&self.height)
                && self.height % 2 == 0,
            "Canvas dimensions must be even and 16–8192"
        );
        ensure!((1..=120).contains(&self.fps), "FPS must be 1–120");
        ensure!(
            (10.0..=540.0).contains(&self.chapter_seconds) && self.chapter_seconds.is_finite(),
            "chapter_seconds must be 10–540"
        );
        ensure!(
            self.lead_in.is_finite()
                && (0.0..=5.0).contains(&self.lead_in)
                && self.tail.is_finite()
                && (0.0..=5.0).contains(&self.tail),
            "lead_in and tail must be 0–5 seconds"
        );
        ensure!(
            self.view.zoom.is_finite()
                && (0.1..=30.0).contains(&self.view.zoom)
                && self.view.x.is_finite()
                && (-12.0..=12.0).contains(&self.view.x)
                && self.view.y.is_finite()
                && (-12.0..=12.0).contains(&self.view.y),
            "Invalid view framing"
        );
        ensure!(
            self.motion_energy.is_finite() && (0.0..=2.0).contains(&self.motion_energy),
            "motion_energy must be 0–2"
        );
        if let Some(model) = &self.tts_model {
            ensure!(!model.trim().is_empty(), "tts_model cannot be empty");
        }
        ensure!(
            (1..=1000).contains(&self.lines.len()),
            "Script requires 1–1000 lines"
        );
        for (index, line) in self.lines.iter().enumerate() {
            ensure!(
                (1..=4000).contains(&line.text.chars().count()),
                "Line {} needs 1–4000 characters",
                index + 1
            );
            ensure!(
                line.pause.is_finite() && (0.0..=5.0).contains(&line.pause),
                "Line {} pause must be 0–5 seconds",
                index + 1
            );
            ensure!(
                line.motion.len() <= 64,
                "Line {} has too many motion cues",
                index + 1
            );
            if let Some(name) = &line.emotion {
                ensure!(
                    EMOTIONS.contains(&name.as_str()),
                    "Line {} has unknown emotion",
                    index + 1
                );
            }
            ensure!(
                line.expression_intensity.is_finite()
                    && (0.0..=5.0).contains(&line.expression_intensity),
                "Line {} expression_intensity must be 0–5",
                index + 1
            );
            if let Some(name) = &line.gesture {
                ensure!(
                    matches!(name.as_str(), "nod" | "shake" | "tilt" | "lean"),
                    "Line {} has unknown gesture",
                    index + 1
                );
            }
            for cue in &line.motion {
                ensure!(
                    cue.at.is_finite()
                        && (0.0..=1.0).contains(&cue.at)
                        && [cue.yaw, cue.pitch, cue.roll]
                            .iter()
                            .all(|v| v.is_finite() && v.abs() <= 90.0),
                    "Line {} has invalid motion cue",
                    index + 1
                );
            }
        }
        Ok(())
    }
}
impl Line {
    pub fn speech_text(&self, provider: &str, model: &str) -> String {
        if provider == "elevenlabs" && model == "eleven_v3" && self.expression_intensity > 0.0 {
            if let Some(tag) = self.emotion.as_deref().and_then(v3_emotion_tag) {
                return format!("{tag} {}", self.text);
            }
        }
        self.text.clone()
    }

    pub fn effective_gesture(&self) -> Option<&str> {
        self.gesture.as_deref().or_else(|| {
            (self.expression_intensity > 0.0)
                .then(|| self.emotion.as_deref().and_then(default_gesture))
                .flatten()
        })
    }

    pub fn head_at(&self, fraction: f64) -> [f32; 3] {
        if self.motion.is_empty() {
            let t = fraction.clamp(0.0, 1.0) as f32;
            let seed = self.text.bytes().fold(0u32, |hash, byte| {
                hash.wrapping_mul(16777619).wrapping_add(byte as u32)
            });
            let phase = (seed % 1000) as f32 / 1000.0 * std::f32::consts::TAU;
            let envelope = (t * std::f32::consts::PI).sin();
            return [
                envelope * (t * std::f32::consts::TAU + phase).sin() * 8.0,
                envelope * (t * std::f32::consts::PI * 1.4 + phase * 0.5).sin() * 4.0,
                envelope * (t * std::f32::consts::TAU * 0.7 + phase).sin() * 2.8,
            ];
        }
        let mut cues = self.motion.clone();
        cues.sort_by(|a, b| a.at.total_cmp(&b.at));
        if fraction <= cues[0].at {
            return cues[0].angles();
        }
        for pair in cues.windows(2) {
            if fraction <= pair[1].at {
                let span = pair[1].at - pair[0].at;
                if span <= 0.0 {
                    return pair[1].angles();
                }
                let t = ((fraction - pair[0].at) / span).clamp(0.0, 1.0) as f32;
                let t = t * t * (3.0 - 2.0 * t);
                let a = pair[0].angles();
                let b = pair[1].angles();
                return [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    a[2] + (b[2] - a[2]) * t,
                ];
            }
        }
        cues.last().unwrap().angles()
    }
}
impl Cue {
    fn angles(self) -> [f32; 3] {
        [self.yaw, self.pitch, self.roll]
    }
}

pub fn validate_output(path: &std::path::Path) -> Result<&'static str> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" => Ok("h264"),
        "webm" => Ok("vp9"),
        "mov" => Ok("prores"),
        _ => bail!("Output must end in .mp4, .webm, or .mov"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn script_rejects_bad_timing_and_interpolates_head() {
        let script = Script::parse(
            r#"{"lines":[{"text":"Hi","motion":[{"at":0,"yaw":-10},{"at":1,"yaw":10}]}]}"#,
        )
        .unwrap();
        assert_eq!(script.lines[0].head_at(0.5)[0], 0.0);
        assert!(Script::parse(r#"{"lines":[{"text":"Hi","pause":-1}]}"#).is_err());
    }
    #[test]
    fn default_head_motion_is_expressive_and_script_controls_are_validated() {
        let script =
            Script::parse(r#"{"lines":[{"text":"Welcome, everyone!","gesture":"nod"}]}"#).unwrap();
        assert_eq!(script.motion_energy, 1.35);
        let peak = (1..20)
            .map(|step| script.lines[0].head_at(step as f64 / 20.0)[0].abs())
            .fold(0.0_f32, f32::max);
        assert!(peak > 4.0);
        assert!(
            script.lines[0]
                .head_at(0.0)
                .iter()
                .all(|angle| angle.abs() < 0.001)
        );
        assert!(
            script.lines[0]
                .head_at(1.0)
                .iter()
                .all(|angle| angle.abs() < 0.001)
        );
        assert!(Script::parse(r#"{"motion_energy":2.1,"lines":[{"text":"Hi"}]}"#).is_err());
        assert!(Script::parse(r#"{"lines":[{"text":"Hi","gesture":"spin"}]}"#).is_err());
    }
    #[test]
    fn eleven_v3_tags_and_model_actions_follow_the_same_emotion() {
        let script = Script::parse(
            r#"{"tts_model":"eleven_v3","lines":[
            {"text":"Hello!","emotion":"excited","expression_intensity":5},
            {"text":"Listen.","emotion":"sad","expression_intensity":0},
            {"text":"Think.","emotion":"thinking","gesture":"nod"}
        ]}"#,
        )
        .unwrap();
        assert_eq!(
            script.lines[0].speech_text("elevenlabs", "eleven_v3"),
            "[excited] Hello!"
        );
        assert_eq!(script.lines[0].effective_gesture(), Some("nod"));
        assert_eq!(
            script.lines[1].speech_text("elevenlabs", "eleven_v3"),
            "Listen."
        );
        assert_eq!(script.lines[2].effective_gesture(), Some("nod"));
        assert_eq!(
            script.lines[0].speech_text("openai", "gpt-4o-mini-tts"),
            "Hello!"
        );
        assert!(Script::parse(r#"{"lines":[{"text":"Hi","expression_intensity":5.1}]}"#).is_err());
    }
}
