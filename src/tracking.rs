use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

pub type Values = BTreeMap<String, f32>;

pub const ARKIT: &[&str] = &[
    "browDownLeft",
    "browDownRight",
    "browInnerUp",
    "browOuterUpLeft",
    "browOuterUpRight",
    "cheekPuff",
    "cheekSquintLeft",
    "cheekSquintRight",
    "eyeBlinkLeft",
    "eyeBlinkRight",
    "eyeLookDownLeft",
    "eyeLookDownRight",
    "eyeLookInLeft",
    "eyeLookInRight",
    "eyeLookOutLeft",
    "eyeLookOutRight",
    "eyeLookUpLeft",
    "eyeLookUpRight",
    "eyeSquintLeft",
    "eyeSquintRight",
    "eyeWideLeft",
    "eyeWideRight",
    "jawForward",
    "jawLeft",
    "jawOpen",
    "jawRight",
    "mouthClose",
    "mouthDimpleLeft",
    "mouthDimpleRight",
    "mouthFrownLeft",
    "mouthFrownRight",
    "mouthFunnel",
    "mouthLeft",
    "mouthLowerDownLeft",
    "mouthLowerDownRight",
    "mouthPressLeft",
    "mouthPressRight",
    "mouthPucker",
    "mouthRight",
    "mouthRollLower",
    "mouthRollUpper",
    "mouthShrugLower",
    "mouthShrugUpper",
    "mouthSmileLeft",
    "mouthSmileRight",
    "mouthStretchLeft",
    "mouthStretchRight",
    "mouthUpperUpLeft",
    "mouthUpperUpRight",
    "noseSneerLeft",
    "noseSneerRight",
    "tongueOut",
];
pub const EXTENSIONS: &[&str] = &[
    "ParamMouthFunnel",
    "ParamMouthPress",
    "ParamMouthShrug",
    "ParamCheekPuff",
    "ParamTongueOut",
    "ParamMouthCornerRound",
    "ParamEyeSquint",
    "ParamBrowDepth",
];

fn canonical(key: &str) -> String {
    if let Some(k) = key.strip_suffix("_L") {
        format!("{k}Left")
    } else if let Some(k) = key.strip_suffix("_R") {
        format!("{k}Right")
    } else {
        key.into()
    }
}

pub fn clean(raw: &Value) -> Result<Values> {
    let obj = raw.as_object().context("Expected parameter object")?;
    let mut result = Values::new();
    for (raw_key, value) in obj {
        let key = canonical(raw_key);
        let angle = matches!(key.as_str(), "yaw" | "pitch" | "roll");
        if !(angle || ARKIT.contains(&key.as_str()) || EXTENSIONS.contains(&key.as_str())) {
            continue;
        }
        let number = match value {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse::<f64>().ok(),
            _ => None,
        }
        .with_context(|| format!("Invalid tracking value for {key}"))?;
        ensure!(number.is_finite(), "Non-finite tracking value for {key}");
        result.insert(
            key,
            number.clamp(
                if angle { -180.0 } else { 0.0 },
                if angle { 180.0 } else { 1.0 },
            ) as f32,
        );
    }
    Ok(result)
}

/// Accepts iFacialMocap pipe packets and the Studio normalized JSON dialect.
pub fn parse(packet: &[u8]) -> Result<Values> {
    ensure!(packet.len() <= 65_536, "Packet too large");
    let text = std::str::from_utf8(packet).context("Invalid tracking UTF-8")?;
    if text.trim_start().starts_with('{') {
        let obj: Value = serde_json::from_str(text).context("Invalid tracking JSON")?;
        let mut raw = obj
            .get("blendshapes")
            .or_else(|| obj.get("parameters"))
            .unwrap_or(&obj)
            .clone();
        if let Some(entries) = raw.as_array() {
            let mut map = serde_json::Map::new();
            for entry in entries {
                let key = entry
                    .get("id")
                    .or_else(|| entry.get("name"))
                    .and_then(Value::as_str)
                    .context("Blendshape lacks id/name")?;
                map.insert(
                    key.into(),
                    entry
                        .get("value")
                        .cloned()
                        .context("Blendshape lacks value")?,
                );
            }
            raw = Value::Object(map);
        }
        let map = raw.as_object_mut().context("Expected parameter object")?;
        if let Some(head) = obj.get("head").and_then(Value::as_object) {
            map.extend(head.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        clean(&raw)
    } else {
        let mut raw = serde_json::Map::new();
        for field in text.split('|') {
            if let Some(head) = field.strip_prefix("=head#") {
                let mut parts = head.split(',');
                for key in ["pitch", "yaw", "roll"] {
                    let value: f64 = parts
                        .next()
                        .context("Invalid head rotation")?
                        .parse()
                        .context("Invalid head rotation")?;
                    raw.insert(key.into(), Value::from(value));
                }
            } else if let Some((key, value)) = field.rsplit_once(['-', '&']) {
                if key.chars().next().is_some_and(char::is_alphabetic)
                    && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && let Ok(value) = value.parse::<f64>()
                {
                    raw.insert(key.into(), Value::from(value / 100.0));
                }
            }
        }
        clean(&Value::Object(raw))
    }
}

#[derive(Debug, Clone)]
pub struct Calibration {
    target: usize,
    pub count: usize,
    samples: HashMap<String, Vec<f32>>,
    pub offsets: Values,
}

impl Calibration {
    pub fn new(samples: usize) -> Self {
        Self {
            target: samples.max(1),
            count: 0,
            samples: HashMap::new(),
            offsets: Values::new(),
        }
    }
    pub fn reset(&mut self) {
        self.count = 0;
        self.samples.clear();
        self.offsets.clear();
    }
    pub fn ready(&self) -> bool {
        self.count >= self.target
    }
    pub fn apply(&mut self, values: &Values) -> Values {
        if !self.ready() {
            for (key, value) in values {
                self.samples.entry(key.clone()).or_default().push(*value);
            }
            self.count += 1;
            if self.ready() {
                for (key, samples) in &mut self.samples {
                    samples.sort_by(f32::total_cmp);
                    self.offsets.insert(key.clone(), samples[samples.len() / 2]);
                }
            }
        }
        values
            .iter()
            .map(|(key, value)| {
                let baseline = self.offsets.get(key).copied().unwrap_or(0.0);
                let adjusted = if matches!(key.as_str(), "yaw" | "pitch" | "roll") {
                    value - baseline
                } else {
                    ((value - baseline) / (1.0 - baseline).max(0.15)).clamp(0.0, 1.0)
                };
                (key.clone(), adjusted)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub id: String,
    pub min: f32,
    pub max: f32,
    #[serde(default)]
    pub default: f32,
}

pub struct Mapper {
    pub schema: BTreeMap<String, Parameter>,
}
impl Mapper {
    pub fn new(parameters: impl IntoIterator<Item = Parameter>) -> Self {
        Self {
            schema: parameters.into_iter().map(|p| (p.id.clone(), p)).collect(),
        }
    }
    pub fn clamp(&self, id: &str, value: f32) -> Option<f32> {
        self.schema.get(id).map(|p| value.clamp(p.min, p.max))
    }
    pub fn defaults(&self) -> Values {
        self.schema
            .iter()
            .map(|(id, p)| (id.clone(), p.default))
            .collect()
    }
    pub fn map(&self, s: &Values) -> Values {
        let get = |key: &str| s.get(key).copied().unwrap_or(0.0);
        let avg = |a: &str, b: &str| (get(a) + get(b)) * 0.5;
        let smile = avg("mouthSmileLeft", "mouthSmileRight");
        let frown = avg("mouthFrownLeft", "mouthFrownRight");
        let funnel = get("mouthFunnel").max(get("ParamMouthFunnel"));
        let pucker = get("mouthPucker").max(get("ParamMouthCornerRound"));
        let press = avg("mouthPressLeft", "mouthPressRight").max(get("ParamMouthPress"));
        let shrug = avg("mouthShrugLower", "mouthShrugUpper").max(get("ParamMouthShrug"));
        let puff = get("cheekPuff").max(get("ParamCheekPuff"));
        let tongue = get("tongueOut").max(get("ParamTongueOut"));
        let mut out = Values::from([
            ("ParamAngleX".into(), get("yaw")),
            ("ParamAngleY".into(), get("pitch")),
            ("ParamAngleZ".into(), get("roll")),
            ("ParamBodyAngleX".into(), get("yaw") * 0.3),
            ("ParamBodyAngleZ".into(), get("roll") * 0.25),
            (
                "ParamEyeLOpen".into(),
                1.0 - get("eyeBlinkLeft") + get("eyeWideLeft") * 0.3,
            ),
            (
                "ParamEyeROpen".into(),
                1.0 - get("eyeBlinkRight") + get("eyeWideRight") * 0.3,
            ),
            ("ParamEyeLSmile".into(), get("cheekSquintLeft")),
            ("ParamEyeRSmile".into(), get("cheekSquintRight")),
            (
                "ParamEyeBallX".into(),
                avg("eyeLookOutLeft", "eyeLookInRight") - avg("eyeLookInLeft", "eyeLookOutRight"),
            ),
            (
                "ParamEyeBallY".into(),
                avg("eyeLookUpLeft", "eyeLookUpRight") - avg("eyeLookDownLeft", "eyeLookDownRight"),
            ),
            (
                "ParamBrowLY".into(),
                get("browInnerUp") + get("browOuterUpLeft") - get("browDownLeft"),
            ),
            (
                "ParamBrowRY".into(),
                get("browInnerUp") + get("browOuterUpRight") - get("browDownRight"),
            ),
            (
                "ParamBrowLAngle".into(),
                get("browOuterUpLeft") - get("browDownLeft"),
            ),
            (
                "ParamBrowRAngle".into(),
                get("browOuterUpRight") - get("browDownRight"),
            ),
            (
                "ParamBrowLForm".into(),
                get("browInnerUp") * 0.3 - get("browDownLeft"),
            ),
            (
                "ParamBrowRForm".into(),
                get("browInnerUp") * 0.3 - get("browDownRight"),
            ),
            (
                "ParamMouthOpenY".into(),
                get("jawOpen") * (1.0 - get("mouthClose")),
            ),
            ("ParamMouthForm".into(), smile - frown),
            ("ParamMouthFunnel".into(), funnel),
            ("ParamMouthPress".into(), press),
            ("ParamMouthShrug".into(), shrug),
            ("ParamCheekPuff".into(), puff),
            ("ParamTongueOut".into(), tongue),
            ("ParamMouthCornerRound".into(), pucker),
            (
                "ParamEyeSquint".into(),
                avg("eyeSquintLeft", "eyeSquintRight").max(get("ParamEyeSquint")),
            ),
            (
                "ParamBrowDepth".into(),
                avg("browDownLeft", "browDownRight").max(get("ParamBrowDepth")),
            ),
        ]);
        if !self.schema.contains_key("ParamMouthFunnel") {
            *out.get_mut("ParamMouthForm").unwrap() -= 0.7 * funnel;
        }
        if !self.schema.contains_key("ParamMouthCornerRound") {
            *out.get_mut("ParamMouthForm").unwrap() -= 0.6 * pucker;
        }
        if !self.schema.contains_key("ParamMouthPress") {
            *out.get_mut("ParamMouthOpenY").unwrap() *= 1.0 - press * 0.7;
        }
        if !self.schema.contains_key("ParamMouthShrug") {
            *out.get_mut("ParamMouthOpenY").unwrap() += shrug * 0.12;
        }
        if !self.schema.contains_key("ParamTongueOut") {
            *out.get_mut("ParamMouthOpenY").unwrap() += tongue * 0.2;
        }
        if !self.schema.contains_key("ParamCheekPuff") {
            *out.get_mut("ParamMouthForm").unwrap() += puff * 0.15;
        }
        if !self.schema.contains_key("ParamEyeSquint") {
            let factor = 1.0 - get("eyeSquintLeft").max(get("ParamEyeSquint")) * 0.35;
            *out.get_mut("ParamEyeLOpen").unwrap() *= factor;
            *out.get_mut("ParamEyeROpen").unwrap() *=
                1.0 - get("eyeSquintRight").max(get("ParamEyeSquint")) * 0.35;
        }
        if !self.schema.contains_key("ParamBrowDepth") {
            *out.get_mut("ParamBrowLY").unwrap() -= get("ParamBrowDepth") * 0.5;
            *out.get_mut("ParamBrowRY").unwrap() -= get("ParamBrowDepth") * 0.5;
        }
        for key in ARKIT {
            let normalized = format!("Param{}{}", key[..1].to_uppercase(), &key[1..]);
            for id in [normalized.as_str(), *key] {
                if self.schema.contains_key(id) && !out.contains_key(id) {
                    out.insert(id.into(), get(key));
                }
            }
        }
        out.into_iter()
            .filter_map(|(id, value)| self.clamp(&id, value).map(|v| (id, v)))
            .collect()
    }
}

pub fn validate_source(source: &str) -> Result<()> {
    if matches!(source, "agent" | "phone" | "webcam") {
        Ok(())
    } else {
        bail!("Unknown tracking source")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packet_dialects_agree() {
        let json = parse(br#"{"blendshapes":{"eyeBlinkLeft":0.7},"head":{"yaw":-12}}"#).unwrap();
        let pipe = parse(b"eyeBlinkLeft-70|=head#0,-12,0").unwrap();
        assert_eq!(json.get("eyeBlinkLeft"), pipe.get("eyeBlinkLeft"));
        assert_eq!(json.get("yaw"), pipe.get("yaw"));
    }
    #[test]
    fn aliases_and_ranges() {
        let values = parse(br#"{"eyeBlink_L":2,"yaw":-999,"unknown":1}"#).unwrap();
        assert_eq!(values.get("eyeBlinkLeft"), Some(&1.0));
        assert_eq!(values.get("yaw"), Some(&-180.0));
        assert!(!values.contains_key("unknown"));
    }
    #[test]
    fn calibration_removes_neutral_pose() {
        let mut calibration = Calibration::new(2);
        let neutral = Values::from([("yaw".into(), 10.0), ("eyeBlinkLeft".into(), 0.2)]);
        calibration.apply(&neutral);
        let result = calibration.apply(&neutral);
        assert_eq!(result["yaw"], 0.0);
        assert_eq!(result["eyeBlinkLeft"], 0.0);
    }
    #[test]
    fn mapper_clamps_model_ranges() {
        let mapper = Mapper::new([Parameter {
            id: "ParamAngleX".into(),
            min: -30.0,
            max: 30.0,
            default: 0.0,
        }]);
        let mapped = mapper.map(&Values::from([("yaw".into(), 180.0)]));
        assert_eq!(mapped["ParamAngleX"], 30.0);
    }
}
