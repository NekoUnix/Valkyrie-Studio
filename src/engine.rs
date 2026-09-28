use crate::tracking::{Calibration, Mapper, Values, clean, validate_source};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub stale_after: f64,
    pub smoothing_seconds: f64,
    pub calibration_samples: usize,
}
impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            stale_after: 0.5,
            smoothing_seconds: 0.045,
            calibration_samples: 45,
        }
    }
}

pub struct Engine {
    pub mode: String,
    pub calibration: Calibration,
    config: EngineConfig,
    inputs: BTreeMap<String, (Values, f64)>,
    direct: BTreeMap<String, (f32, f64)>,
    emotion: Option<(Values, f64)>,
    filtered: BTreeMap<String, Values>,
    pub agent_motion_energy: f32,
    gesture: Option<AgentGesture>,
}

#[derive(Clone, Copy)]
struct AgentGesture {
    kind: GestureKind,
    start: f64,
    duration: f64,
    intensity: f32,
}

#[derive(Clone, Copy)]
enum GestureKind {
    Nod,
    Shake,
    Tilt,
    Lean,
}

impl AgentGesture {
    fn offsets(self, time: f64) -> [f32; 5] {
        let progress = ((time - self.start) / self.duration).clamp(0.0, 1.0) as f32;
        let envelope = (progress * std::f32::consts::PI).sin().max(0.0) * self.intensity;
        let beat = (progress * std::f32::consts::TAU).sin() * envelope;
        match self.kind {
            GestureKind::Nod => [0.0, beat * 14.0, 0.0, 0.0, 0.0],
            GestureKind::Shake => [beat * 16.0, 0.0, 0.0, 0.0, 0.0],
            GestureKind::Tilt => [0.0, 0.0, envelope * 11.0, 0.0, envelope * 3.0],
            GestureKind::Lean => [0.0, 0.0, envelope * 2.0, envelope * 8.0, envelope * 7.0],
        }
    }
}

impl Engine {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            mode: "agent".into(),
            calibration: Calibration::new(config.calibration_samples),
            config,
            inputs: BTreeMap::new(),
            direct: BTreeMap::new(),
            emotion: None,
            filtered: BTreeMap::new(),
            agent_motion_energy: 1.0,
            gesture: None,
        }
    }
    pub fn set_mode(&mut self, mode: &str) -> Result<()> {
        if !matches!(mode, "agent" | "phone" | "webcam" | "idle") {
            bail!("Mode must be agent, phone, webcam or idle")
        }
        self.mode = mode.into();
        self.calibration.reset();
        self.filtered.clear();
        if mode != "agent" {
            self.gesture = None;
        }
        Ok(())
    }
    pub fn set_agent_motion_energy(&mut self, energy: f32) -> Result<()> {
        ensure!(
            energy.is_finite() && (0.0..=2.0).contains(&energy),
            "Agent motion energy must be 0–2"
        );
        self.agent_motion_energy = energy;
        Ok(())
    }
    pub fn gesture(&mut self, name: &str, intensity: f32, duration: f64, now: f64) -> Result<()> {
        ensure!(
            self.mode == "agent",
            "Select agent mode before sending a gesture"
        );
        let kind = match name {
            "nod" => GestureKind::Nod,
            "shake" => GestureKind::Shake,
            "tilt" => GestureKind::Tilt,
            "lean" => GestureKind::Lean,
            _ => bail!("Gesture must be nod, shake, tilt or lean"),
        };
        ensure!(
            intensity.is_finite() && (0.0..=2.0).contains(&intensity),
            "Gesture intensity must be 0–2"
        );
        ensure!(
            duration.is_finite() && (0.2..=5.0).contains(&duration),
            "Gesture duration must be 0.2–5 seconds"
        );
        self.gesture = Some(AgentGesture {
            kind,
            start: now,
            duration,
            intensity,
        });
        Ok(())
    }
    pub fn ingest(&mut self, source: &str, values: &Value, time: f64) -> Result<()> {
        validate_source(source)?;
        let mut values = clean(values)?;
        if source == self.mode && matches!(source, "phone" | "webcam") {
            values = self.calibration.apply(&values);
        }
        self.inputs.insert(source.into(), (values, time));
        Ok(())
    }
    pub fn ingest_clean(&mut self, source: &str, mut values: Values, time: f64) -> Result<()> {
        validate_source(source)?;
        if source == self.mode && matches!(source, "phone" | "webcam") {
            values = self.calibration.apply(&values);
        }
        self.inputs.insert(source.into(), (values, time));
        Ok(())
    }
    pub fn emotion(&mut self, name: &str, intensity: f32, until: f64) -> Result<()> {
        ensure!(
            (0.0..=1.0).contains(&intensity) && intensity.is_finite(),
            "Invalid emotion intensity"
        );
        let values: &[(&str, f32)] = match name {
            "joy" => &[
                ("mouthSmileLeft", 0.85),
                ("mouthSmileRight", 0.85),
                ("cheekSquintLeft", 0.4),
                ("cheekSquintRight", 0.4),
            ],
            "thinking" => &[("yaw", -10.0), ("roll", 7.0), ("browOuterUpRight", 0.5)],
            "angry" => &[
                ("browDownLeft", 0.8),
                ("browDownRight", 0.8),
                ("mouthFrownLeft", 0.6),
                ("mouthFrownRight", 0.6),
            ],
            "surprised" => &[
                ("jawOpen", 0.7),
                ("eyeWideLeft", 0.8),
                ("eyeWideRight", 0.8),
                ("browInnerUp", 0.8),
            ],
            "neutral" => &[],
            _ => bail!("Unknown emotion"),
        };
        self.emotion = Some((
            values
                .iter()
                .map(|(k, v)| ((*k).into(), v * intensity))
                .collect(),
            until,
        ));
        Ok(())
    }
    pub fn parameters(&mut self, values: &Values, until: f64) -> Result<()> {
        ensure!(
            values.len() <= 512,
            "parameters must be an object with at most 512 IDs"
        );
        ensure!(
            values.values().all(|v| v.is_finite()),
            "Non-finite parameter override"
        );
        self.direct
            .extend(values.iter().map(|(k, v)| (k.clone(), (*v, until))));
        Ok(())
    }
    pub fn clear_parameters(&mut self) {
        self.direct.clear();
    }
    pub fn sample(
        &mut self,
        mapper: &Mapper,
        time: f64,
        dt: f64,
        mouth: Option<&Values>,
        key: &str,
    ) -> Values {
        let mut values = self
            .inputs
            .get(&self.mode)
            .filter(|(_, stamp)| time - stamp <= self.config.stale_after)
            .map(|(v, _)| v.clone())
            .unwrap_or_default();
        if let Some((emotion, until)) = &self.emotion {
            if time < *until {
                for (id, value) in emotion {
                    if matches!(id.as_str(), "yaw" | "pitch" | "roll") {
                        *values.entry(id.clone()).or_insert(0.0) += value;
                    } else {
                        values.insert(id.clone(), *value);
                    }
                }
            }
        }
        let agent_motion = self.mode == "agent" && self.agent_motion_energy > 0.0;
        let has_eye_tracking = [
            "eyeLookOutLeft",
            "eyeLookInLeft",
            "eyeLookOutRight",
            "eyeLookInRight",
            "eyeLookUpLeft",
            "eyeLookUpRight",
            "eyeLookDownLeft",
            "eyeLookDownRight",
        ]
        .iter()
        .any(|id| values.contains_key(*id));
        if agent_motion {
            let energy = self.agent_motion_energy;
            let ambient = [
                (time * 0.91).sin() * 4.0 + (time * 0.37 + 1.1).sin() * 2.0,
                (time * 0.67 + 0.7).sin() * 2.5 + (time * 0.31).sin(),
                (time * 0.72 + 1.3).sin() * 2.2,
            ];
            for (id, offset) in ["yaw", "pitch", "roll"].into_iter().zip(ambient) {
                let amount = if values.contains_key(id) { 0.28 } else { 1.0 };
                *values.entry(id.into()).or_insert(0.0) += offset as f32 * energy * amount;
            }
        }
        let gesture = self
            .gesture
            .filter(|active| time < active.start + active.duration);
        self.gesture = gesture;
        let gesture_offsets = if self.mode == "agent" {
            gesture
                .map(|active| active.offsets(time))
                .unwrap_or([0.0; 5])
        } else {
            [0.0; 5]
        };
        if gesture.is_some() && self.mode == "agent" {
            for (id, offset) in ["yaw", "pitch", "roll"].into_iter().zip(gesture_offsets) {
                *values.entry(id.into()).or_insert(0.0) += offset;
            }
        }
        let mut desired = mapper.defaults();
        desired.extend(mapper.map(&values));
        if self.mode == "agent" {
            for (id, offset) in [
                ("ParamBodyAngleX", gesture_offsets[3]),
                ("ParamBodyAngleZ", gesture_offsets[4]),
            ] {
                if mapper.schema.contains_key(id) {
                    *desired.entry(id.into()).or_insert(0.0) += offset;
                }
            }
        }
        if agent_motion {
            let energy = self.agent_motion_energy;
            for (id, offset) in [
                ("ParamBodyAngleX", (time * 0.59 + 0.4).sin() * 2.2),
                ("ParamBodyAngleY", (time * 0.51 + 1.2).sin() * 1.4),
                ("ParamBodyAngleZ", (time * 0.46 + 2.0).sin() * 1.8),
            ] {
                if mapper.schema.contains_key(id) {
                    *desired.entry(id.into()).or_insert(0.0) += offset as f32 * energy;
                }
            }
            if !has_eye_tracking {
                for (id, gaze) in [
                    (
                        "ParamEyeBallX",
                        (time * 0.57).sin() * 0.34 + (time * 0.23).sin() * 0.12,
                    ),
                    ("ParamEyeBallY", (time * 0.39 + 0.8).sin() * 0.22),
                ] {
                    if mapper.schema.contains_key(id) {
                        desired.insert(id.into(), gaze as f32 * energy.min(1.5));
                    }
                }
            }
            let phase = time.rem_euclid(4.6);
            let blink = if phase < 0.17 {
                ((phase / 0.085) - 1.0).abs() as f32
            } else {
                1.0
            };
            for (id, input) in [
                ("ParamEyeLOpen", "eyeBlinkLeft"),
                ("ParamEyeROpen", "eyeBlinkRight"),
            ] {
                if mapper.schema.contains_key(id)
                    && !values.contains_key(input)
                    && let Some(open) = desired.get_mut(id)
                {
                    *open *= blink;
                }
            }
        }
        if mapper.schema.contains_key("ParamBreath") {
            desired.insert(
                "ParamBreath".into(),
                ((time * 1.8).sin() as f32 + 1.0) * 0.5,
            );
        }
        if values.is_empty() && self.mode != "agent" {
            if mapper.schema.contains_key("ParamAngleZ") {
                desired.insert("ParamAngleZ".into(), (time * 0.7).sin() as f32 * 1.2);
            }
            let phase = time % 4.7;
            let blink = if phase < 0.15 {
                ((phase / 0.075) - 1.0).abs() as f32
            } else {
                1.0
            };
            for id in ["ParamEyeLOpen", "ParamEyeROpen"] {
                if mapper.schema.contains_key(id) {
                    desired.insert(id.into(), blink);
                }
            }
        }
        if let Some(mouth) = mouth {
            desired.extend(
                mouth
                    .iter()
                    .filter(|(id, _)| mapper.schema.contains_key(*id))
                    .map(|(id, v)| (id.clone(), *v)),
            );
        }
        self.direct.retain(|_, (_, until)| time < *until);
        desired.extend(
            self.direct
                .iter()
                .filter(|(id, _)| mapper.schema.contains_key(*id))
                .map(|(id, (v, _))| (id.clone(), *v)),
        );
        let previous = self
            .filtered
            .entry(key.into())
            .or_insert_with(|| mapper.defaults());
        let alpha =
            1.0 - (-dt.max(0.0) / self.config.smoothing_seconds.clamp(0.001, 1.0)).exp() as f32;
        for (id, value) in desired {
            if let Some(value) = mapper.clamp(&id, value) {
                let old = previous.get(&id).copied().unwrap_or(value);
                previous.insert(id, old + (value - old) * alpha);
            }
        }
        previous.clone()
    }
}

pub struct Timeline {
    pub duration: f64,
    events: Vec<(f64, usize, Value)>,
    cursor: usize,
}
impl Timeline {
    pub fn parse(data: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(data)?;
        let duration = value["duration"]
            .as_f64()
            .context("Timeline duration missing")?;
        ensure!(
            (0.01..=86_400.0).contains(&duration),
            "Invalid timeline duration"
        );
        let source = value["events"]
            .as_array()
            .context("Timeline events missing")?;
        let mut events = Vec::with_capacity(source.len());
        for (index, event) in source.iter().enumerate() {
            let time = event["time"]
                .as_f64()
                .context("Timeline event time missing")?;
            ensure!(
                time >= 0.0 && time <= duration,
                "Invalid timeline event time"
            );
            ensure!(event["command"].is_object(), "Timeline command missing");
            events.push((time, index, event["command"].clone()));
        }
        events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        Ok(Self {
            duration,
            events,
            cursor: 0,
        })
    }
    pub fn due(&mut self, time: f64) -> Vec<Value> {
        let mut due = Vec::new();
        while self.cursor < self.events.len() && self.events[self.cursor].0 <= time + 1e-9 {
            due.push(self.events[self.cursor].2.clone());
            self.cursor += 1;
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::Parameter;
    #[test]
    fn direct_override_expires_without_discontinuity() {
        let mapper = Mapper::new([Parameter {
            id: "ParamAngleX".into(),
            min: -30.0,
            max: 30.0,
            default: 0.0,
        }]);
        let mut engine = Engine::new(EngineConfig::default());
        engine
            .parameters(&Values::from([("ParamAngleX".into(), 20.0)]), 1.0)
            .unwrap();
        let during = engine.sample(&mapper, 0.5, 0.1, None, "primary")["ParamAngleX"];
        let after = engine.sample(&mapper, 1.1, 0.1, None, "primary")["ParamAngleX"];
        assert!(during > after && after > 0.0);
    }
    #[test]
    fn timeline_is_stable_for_equal_times() {
        let mut timeline = Timeline::parse(r#"{"duration":2,"events":[{"time":1,"command":{"op":"first"}},{"time":1,"command":{"op":"second"}}]}"#).unwrap();
        assert!(timeline.due(0.9).is_empty());
        let due = timeline.due(1.0);
        assert_eq!(due[0]["op"], "first");
        assert_eq!(due[1]["op"], "second");
    }
    #[test]
    fn agent_blinks_while_head_tracking_is_active() {
        let mapper = Mapper::new([
            Parameter {
                id: "ParamAngleX".into(),
                min: -30.0,
                max: 30.0,
                default: 0.0,
            },
            Parameter {
                id: "ParamEyeLOpen".into(),
                min: 0.0,
                max: 1.0,
                default: 1.0,
            },
            Parameter {
                id: "ParamEyeROpen".into(),
                min: 0.0,
                max: 1.0,
                default: 1.0,
            },
        ]);
        let mut engine = Engine::new(EngineConfig::default());
        engine
            .ingest("agent", &serde_json::json!({"yaw":10}), 0.0)
            .unwrap();
        let closed = engine.sample(&mapper, 0.085, 1.0, None, "primary");
        let open = engine.sample(&mapper, 0.3, 1.0, None, "primary");
        assert!(closed["ParamEyeLOpen"] < 0.05);
        assert!(open["ParamEyeLOpen"] > 0.95);
        assert!(open["ParamAngleX"] > 0.0);
    }
    #[test]
    fn gesture_adds_motion_then_releases_and_energy_can_be_disabled() {
        let mapper = Mapper::new([Parameter {
            id: "ParamAngleY".into(),
            min: -30.0,
            max: 30.0,
            default: 0.0,
        }]);
        let mut engine = Engine::new(EngineConfig::default());
        engine.set_agent_motion_energy(0.0).unwrap();
        engine.gesture("nod", 1.0, 1.0, 0.0).unwrap();
        let nod = engine.sample(&mapper, 0.25, 1.0, None, "primary");
        let settled = engine.sample(&mapper, 1.1, 1.0, None, "primary");
        assert!(nod["ParamAngleY"] > 8.0);
        assert!(settled["ParamAngleY"].abs() < 0.1);
        assert!(engine.set_agent_motion_energy(2.1).is_err());
        assert!(engine.gesture("unknown", 1.0, 1.0, 0.0).is_err());
    }
    #[test]
    fn thinking_emotion_layers_on_agent_head_direction() {
        let mapper = Mapper::new([Parameter {
            id: "ParamAngleX".into(),
            min: -30.0,
            max: 30.0,
            default: 0.0,
        }]);
        let mut engine = Engine::new(EngineConfig::default());
        engine.set_agent_motion_energy(0.0).unwrap();
        engine
            .ingest("agent", &serde_json::json!({"yaw":15}), 0.0)
            .unwrap();
        engine.emotion("thinking", 1.0, 1.0).unwrap();
        let sample = engine.sample(&mapper, 0.2, 1.0, None, "primary");
        assert!((sample["ParamAngleX"] - 5.0).abs() < 0.01);
    }
}
