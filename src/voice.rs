//! Text-to-speech clients. Credentials are never included in public status or
//! provider error messages. On Windows, saved keys use current-user DPAPI.
use anyhow::{Context, Result, bail, ensure};
use reqwest::{blocking::Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, io::Read, path::Path, time::Duration};

pub const OPENAI_VOICES: &[&str] = &["alloy","ash","ballad","coral","echo","fable","nova","onyx","sage","shimmer","verse","marin","cedar"];
pub const OPENAI_MODELS: &[&str] = &["gpt-4o-mini-tts","gpt-4o-mini-tts-2025-12-15","tts-1","tts-1-hd"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceChoice { pub voice_id: String, pub name: String }

#[derive(Clone, Debug)]
pub struct VoiceConfig {
    pub openai_key: Option<String>,
    pub elevenlabs_key: Option<String>,
    pub openai_voice: String,
    pub openai_model: String,
    pub elevenlabs_voice: String,
    pub elevenlabs_model: String,
    pub speed: f32,
    pub instructions: String,
}
impl Default for VoiceConfig {
    fn default() -> Self {
        Self { openai_key: std::env::var("OPENAI_API_KEY").ok().filter(|v| !v.is_empty()),
            elevenlabs_key: std::env::var("ELEVENLABS_API_KEY").ok().filter(|v| !v.is_empty()),
            openai_voice: "coral".into(), openai_model: "gpt-4o-mini-tts".into(),
            elevenlabs_voice: std::env::var("ELEVENLABS_VOICE_ID").unwrap_or_default(),
            elevenlabs_model: "eleven_multilingual_v2".into(), speed: 1.0,
            instructions: "Speak naturally, warmly and clearly.".into() }
    }
}
impl VoiceConfig {
    pub fn load_saved(root: &Path) -> Self {
        let mut config = Self::default();
        if config.openai_key.is_none() { config.openai_key = read_saved_key(&root.join(".credentials/openai-key.dpapi")).ok(); }
        if config.elevenlabs_key.is_none() { config.elevenlabs_key = read_saved_key(&root.join(".credentials/elevenlabs-key.dpapi")).ok(); }
        config
    }
    pub fn public_status(&self) -> Value {
        json!({"openai":{"configured":self.openai_key.is_some(),"voice":self.openai_voice,"model":self.openai_model,"voices":OPENAI_VOICES},
            "elevenlabs":{"configured":self.elevenlabs_key.is_some(),"voice_id":self.elevenlabs_voice,"model":self.elevenlabs_model}})
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(OPENAI_MODELS.contains(&self.openai_model.as_str()), "Unsupported OpenAI speech model");
        ensure!(OPENAI_VOICES.contains(&self.openai_voice.as_str()), "Unknown OpenAI voice");
        ensure!((0.25..=4.0).contains(&self.speed) && self.speed.is_finite(), "OpenAI speech speed must be 0.25–4");
        ensure!(self.instructions.len() <= 2000, "Voice instructions are too long");
        ensure!(self.elevenlabs_model.len() <= 80 && valid_id(&self.elevenlabs_model), "Invalid ElevenLabs model ID");
        ensure!(self.elevenlabs_voice.is_empty() || valid_id(&self.elevenlabs_voice), "Invalid ElevenLabs voice ID");
        Ok(())
    }
    pub fn set_key(&mut self, provider: &str, key: &str, root: &Path, remember: bool) -> Result<()> {
        ensure!((16..=512).contains(&key.len()) && !key.chars().any(char::is_whitespace), "Invalid API key");
        match provider {
            "openai" => {
                ensure!(key.starts_with("sk-"), "Invalid OpenAI API key");
                if remember { save_key(&root.join(".credentials/openai-key.dpapi"), key)?; }
                self.openai_key = Some(key.into());
            }
            "elevenlabs" => {
                if remember { save_key(&root.join(".credentials/elevenlabs-key.dpapi"), key)?; }
                self.elevenlabs_key = Some(key.into());
            }
            _ => bail!("Unknown voice provider"),
        }
        Ok(())
    }
    pub fn forget_key(&mut self, provider: &str, root: &Path) -> Result<()> {
        match provider {
            "openai" => { self.openai_key = None; let _=fs::remove_file(root.join(".credentials/openai-key.dpapi")); }
            "elevenlabs" => { self.elevenlabs_key = None; let _=fs::remove_file(root.join(".credentials/elevenlabs-key.dpapi")); }
            _ => bail!("Unknown voice provider"),
        }
        Ok(())
    }
}

fn valid_id(value: &str) -> bool { !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') }
fn client() -> Result<Client> {
    Ok(Client::builder().connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90)).build()?)
}
fn response_error(status: StatusCode) -> anyhow::Error {
    let message = match status.as_u16() {
        400 => "Speech settings were rejected; check model, voice and text.",
        401 => "The API key was rejected.",
        403 => "This key lacks access to the requested speech feature.",
        429 => "The provider rate limit or quota was reached.",
        _ => "Speech service request failed.",
    };
    anyhow::anyhow!("{message} (HTTP {})", status.as_u16())
}
fn limited_body(response: reqwest::blocking::Response, limit: u64) -> Result<Vec<u8>> {
    ensure!(response.status().is_success(), "{}", response_error(response.status()));
    if let Some(len) = response.content_length() { ensure!(len <= limit, "Voice response too large"); }
    let mut bytes = Vec::new();
    response.take(limit+1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "Voice response too large");
    Ok(bytes)
}

pub fn list_elevenlabs_voices(config: &VoiceConfig) -> Result<Vec<VoiceChoice>> {
    let key = config.elevenlabs_key.as_deref().context("Add an ElevenLabs API key first")?;
    let client = client()?;
    let mut voices = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..50 {
        let mut url = reqwest::Url::parse("https://api.elevenlabs.io/v2/voices")?;
        url.query_pairs_mut().append_pair("page_size", "100");
        if let Some(next) = &token { url.query_pairs_mut().append_pair("next_page_token", next); }
        let response = client.get(url).header("xi-api-key", key).send()?;
        let page: Value = serde_json::from_slice(&limited_body(response, 5*1024*1024)?)
            .context("ElevenLabs returned an invalid voice list")?;
        let entries = page["voices"].as_array().context("ElevenLabs returned an invalid voice list")?;
        for entry in entries {
            let Some(id) = entry["voice_id"].as_str() else { continue };
            let Some(name) = entry["name"].as_str() else { continue };
            if valid_id(id) && !name.is_empty() && !voices.iter().any(|v: &VoiceChoice| v.voice_id == id) {
                voices.push(VoiceChoice { voice_id:id.into(), name:name.into() });
            }
        }
        ensure!(voices.len() <= 5000, "ElevenLabs voice list is too large");
        if page["has_more"] != true {
            voices.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.voice_id.cmp(&b.voice_id)));
            return Ok(voices)
        }
        token = page["next_page_token"].as_str().map(str::to_owned);
        ensure!(token.as_deref().is_some_and(|v| !v.is_empty()), "ElevenLabs voice pagination failed");
    }
    bail!("ElevenLabs voice list exceeded 50 pages")
}

pub fn synthesize(provider: &str, text: &str, config: &VoiceConfig) -> Result<(Vec<u8>, &'static str)> {
    ensure!((1..=4000).contains(&text.chars().count()), "Speech text must contain 1–4000 characters");
    config.validate()?;
    let client = client()?;
    let (url, key, body, suffix) = match provider {
        "openai" => {
            let key = config.openai_key.as_deref().context("Add an OpenAI API key first")?;
            let mut body = json!({"model":config.openai_model,"input":text,"voice":config.openai_voice,"speed":config.speed,"response_format":"wav"});
            if config.openai_model.starts_with("gpt-4o-mini-tts") && !config.instructions.is_empty() {
                body["instructions"] = Value::String(config.instructions.clone());
            }
            ("https://api.openai.com/v1/audio/speech".to_owned(), format!("Bearer {key}"), body, "wav")
        }
        "elevenlabs" => {
            let key = config.elevenlabs_key.as_deref().context("Add an ElevenLabs API key first")?;
            ensure!(valid_id(&config.elevenlabs_voice), "Select an ElevenLabs voice first");
            (format!("https://api.elevenlabs.io/v1/text-to-speech/{}?output_format=mp3_44100_128", config.elevenlabs_voice),
                key.into(), json!({"text":text,"model_id":config.elevenlabs_model}), "mp3")
        }
        _ => bail!("TTS provider must be openai or elevenlabs"),
    };
    let request = client.post(url).json(&body);
    let request = if provider == "openai" { request.header("Authorization", key) } else { request.header("xi-api-key", key) };
    let bytes = limited_body(request.send()?, 64*1024*1024)?;
    ensure!(!bytes.is_empty(), "Speech service returned empty audio");
    Ok((bytes, suffix))
}

fn read_saved_key(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    ensure!(bytes.len() <= 65_536, "Saved key is invalid");
    Ok(String::from_utf8(protect(&bytes, true)?)?)
}
fn save_key(path: &Path, key: &str) -> Result<()> {
    let encrypted = protect(key.as_bytes(), false)?;
    fs::create_dir_all(path.parent().context("Credential path has no parent")?)?;
    fs::write(path, encrypted)?;
    Ok(())
}
#[cfg(windows)]
fn protect(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>> {
    use windows::{Win32::{Foundation::{HLOCAL, LocalFree}, Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    }}, core::w};
    ensure!(bytes.len() <= 49_152, "Credential is too large");
    let input = CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr().cast_mut() };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if decrypt { CryptUnprotectData(&input,None,None,None,None,CRYPTPROTECT_UI_FORBIDDEN,&mut output) }
        else { CryptProtectData(&input,w!("Valkyrie Studio voice key"),None,None,None,CRYPTPROTECT_UI_FORBIDDEN,&mut output) }
            .context("Windows could not protect or unlock this voice key")?;
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        if decrypt { for i in 0..output.cbData as usize { std::ptr::write_volatile(output.pbData.add(i), 0); } }
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(result)
    }
}
#[cfg(not(windows))]
fn protect(_: &[u8], _: bool) -> Result<Vec<u8>> {
    bail!("Saved voice keys currently require Windows DPAPI; use session keys or environment variables")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_status_excludes_keys() {
        let mut config = VoiceConfig::default();
        config.openai_key=Some("private-openai".into()); config.elevenlabs_key=Some("private-eleven".into());
        let status=config.public_status().to_string();
        assert!(!status.contains("private-openai") && !status.contains("private-eleven"));
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires locally saved Windows credentials"]
    fn decrypts_existing_windows_key() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let result = read_saved_key(&root.join(".credentials/elevenlabs-key.dpapi"));
        assert!(result.is_ok(), "Saved voice key could not be unlocked: {:?}", result.err());
    }
}
