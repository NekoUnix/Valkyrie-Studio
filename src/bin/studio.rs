#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{egui, egui_wgpu::RenderState};
use serde_json::{Value, json};
use std::{env, fs, path::{Path, PathBuf}, sync::mpsc::{self, Receiver, Sender}, thread, time::{Duration, Instant}};
use valkyrie_studio::{app_paths, engine::{Engine, EngineConfig}, model::ModelAssets,
    network::{Inbox, Network, NetworkConfig, new_token}, purism::CubismModel,
    renderer::ModelRenderer, physics::Physics, tracking::{Mapper, Parameter, Values},
    audio::{AudioClip, AudioPlayer}, export::Export, guides, voice::{self, VoiceChoice, VoiceConfig}};

enum VoiceEvent {
    Voices(Result<Vec<VoiceChoice>, String>),
    Speech(Result<(AudioClip, PathBuf, bool), String>),
}

fn main() -> eframe::Result {
    let mut args = env::args_os().skip(1);
    let mut startup_model = None;
    while let Some(arg) = args.next() {
        if arg == "--model" { startup_model = args.next().map(PathBuf::from); }
        else if arg == "--help" {
            println!("Usage: valkyrie-studio [--model PATH_TO_MODEL3_JSON]");
            return Ok(());
        }
    }
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if cfg!(windows) && let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::DX12);
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Valkyrie Studio — Rust preview")
            .with_app_id("com.nekounix.valkyrie-studio")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native("Valkyrie Studio", options, Box::new(move |cc| {
        Ok(Box::new(Studio::new(cc, startup_model.take())))
    }))
}

struct Studio {
    render_state: Option<RenderState>,
    model: Option<CubismModel>,
    renderer: Option<ModelRenderer>,
    physics: Option<Physics>,
    mapper: Option<Mapper>,
    engine: Engine,
    inbox: Inbox,
    network: Option<Network>,
    model_path: Option<PathBuf>,
    data_dir: PathBuf,
    path_field: String,
    startup_model: Option<PathBuf>,
    notice: String,
    zoom: f32,
    pan: egui::Vec2,
    ui_scale: f32,
    guide: &'static str,
    show_guides: bool,
    guide_margins: [f32; 4],
    canvas: [u32; 2],
    last_frame: Instant,
    started: Instant,
    command_count: u64,
    voice: VoiceConfig,
    voices: Vec<VoiceChoice>,
    voice_events: Sender<VoiceEvent>,
    voice_results: Receiver<VoiceEvent>,
    voice_busy: bool,
    speech_text: String,
    voice_provider: &'static str,
    key_field: String,
    remember_key: bool,
    audio_player: Option<AudioPlayer>,
    audio_clip: Option<AudioClip>,
    audio_path: Option<PathBuf>,
    export: Option<Export>,
    record_output: String,
    record_codec: &'static str,
    frame_count: u32,
    fps_since: Instant,
    fps: f32,
    model_ms: f32,
    render_ms: f32,
    capture_ms: f32,
}

impl Studio {
    fn new(cc: &eframe::CreationContext<'_>, startup_model: Option<PathBuf>) -> Self {
        let inbox = Inbox::new();
        let mut notice = "Load a local .model3.json to begin.".to_owned();
        let data_dir = app_paths::data_dir().unwrap_or_else(|error| {
            notice = format!("User data path error: {error}");
            env::temp_dir().join("ValkyrieStudio")
        });
        let mut network = None;
        let token = env::var("L2D_API_TOKEN").ok().filter(|s| !s.is_empty()).or_else(|| new_token().ok());
        if let Some(token) = token {
            let mut config = NetworkConfig::default();
            if let Ok(port) = env::var("L2D_API_PORT") {
                if let Ok(port) = port.parse() { config.api.set_port(port); }
            }
            match Network::start(config, &inbox, token.clone()) {
                Ok(active) => {
                    let file = data_dir.join("tmp")
                        .join(if active.api_address.port() == 4141 { "api-token".into() }
                              else { format!("api-token-{}", active.api_address.port()) });
                    if fs::create_dir_all(file.parent().unwrap()).and_then(|_| fs::write(&file, token)).is_ok() {
                        network = Some(active);
                    } else { notice = "Could not write the local agent token; API is disabled.".into(); }
                }
                Err(error) => notice = format!("Agent API unavailable: {error}"),
            }
        }
        let (voice_events, voice_results) = mpsc::channel();
        let voice = VoiceConfig::load_saved(&data_dir);
        Self { render_state: cc.wgpu_render_state.clone(), model: None, renderer: None, physics: None,
            mapper: None, engine: Engine::new(EngineConfig::default()), inbox, network,
            model_path: None, data_dir, path_field: startup_model.as_ref().map_or(String::new(), |p| p.display().to_string()),
            startup_model, notice, zoom: 1.0, pan: egui::Vec2::ZERO, ui_scale: 1.0,
            guide: "All platforms", show_guides: true, guide_margins: [0.06, 0.12, 0.2, 0.25],
            canvas: [1080, 1920], last_frame: Instant::now(),
            started: Instant::now(), command_count: 0,
            voice, voices: Vec::new(), voice_events, voice_results,
            voice_busy: false, speech_text: String::new(), voice_provider: "elevenlabs",
            key_field: String::new(), remember_key: cfg!(windows), audio_player: None,
            audio_clip: None, audio_path: None, export: None,
            record_output: String::new(), record_codec: "h264", frame_count: 0,
            fps_since: Instant::now(), fps: 0.0, model_ms: 0.0, render_ms: 0.0, capture_ms: 0.0 }
    }

    fn load_model(&mut self, path: &Path) -> anyhow::Result<()> {
        anyhow::ensure!(!self.is_recording(), "Stop recording before changing models");
        let assets = ModelAssets::open(path)?;
        let mut model = CubismModel::load(Path::new(""), &assets.moc, assets.textures.len())?;
        model.update()?;
        let state = self.render_state.as_ref().ok_or_else(|| anyhow::anyhow!("WGPU is unavailable"))?;
        let mut renderer = ModelRenderer::new_sized(state, model.canvas, &model.drawables, &assets.textures, self.canvas[0], self.canvas[1])?;
        renderer.render(model.canvas, &model.drawables)?;
        let mapper = Mapper::new(model.parameters().iter().map(|p| Parameter {
            id: p.id.clone(), min: p.min, max: p.max, default: p.default,
        }));
        let physics = assets.physics.as_ref().map(|definition| Physics::load(definition, model.parameters())).transpose()?;
        self.notice = format!("Loaded {} with {} parameters and {} atlases.",
            assets.manifest.file_name().unwrap_or_default().to_string_lossy(),
            model.parameters().len(), assets.textures.len());
        self.model_path = Some(assets.manifest);
        self.renderer = Some(renderer);
        self.physics = physics;
        self.mapper = Some(mapper);
        self.model = Some(model);
        self.engine = Engine::new(EngineConfig::default());
        Ok(())
    }

    fn status(&self) -> Value {
        json!({
            "model": self.model_path.as_ref().and_then(|p| p.file_stem()).map(|n| n.to_string_lossy().to_string()),
            "model_path": self.model_path.as_ref().map(|p| p.display().to_string()),
            "mode": self.engine.mode,
            "agent": {"ready": self.model.is_some() && self.engine.mode == "agent", "commands": self.command_count},
            "view": {"zoom": self.zoom, "x": self.pan.x, "y": self.pan.y},
            "canvas": {"width": self.canvas[0], "height": self.canvas[1]},
            "guides": {"preset": self.guide, "enabled": self.show_guides,
                "margins": guides::margins(self.guide, self.guide_margins)},
            "notice": self.notice,
            "voice": self.voice.public_status(),
            "voice_busy": self.voice_busy,
            "audio_path": self.audio_path.as_ref().map(|p| p.display().to_string()),
            "audio_duration": self.audio_clip.as_ref().map(|clip| clip.duration),
            "recording": self.export.as_ref().map(Export::stats),
            "runtime": "Rust + Purism Core"
            ,"performance": {"fps": self.fps, "model_ms": self.model_ms, "render_ms": self.render_ms,
                "capture_ms": self.capture_ms}
        })
    }

    fn command(&mut self, request: &Value) -> anyhow::Result<Value> {
        let op = request["op"].as_str().ok_or_else(|| anyhow::anyhow!("Missing op"))?;
        let now = self.started.elapsed().as_secs_f64();
        self.command_count += 1;
        match op {
            "status" => Ok(self.status()),
            "schema" => {
                let model = self.model.as_ref().ok_or_else(|| anyhow::anyhow!("Load a model first"))?;
                Ok(json!({"parameters": model.parameters(), "drawables": model.drawables.iter().map(|d| &d.id).collect::<Vec<_>>(), "canvas": model.canvas}))
            }
            "mode" => { self.engine.set_mode(request["mode"].as_str().unwrap_or(""))?; Ok(json!(true)) }
            "calibrate" => { self.engine.calibration.reset(); Ok(json!(true)) }
            "tracking" => {
                let source = request["source"].as_str().unwrap_or("agent");
                self.engine.ingest(source, &request["values"], now)?;
                Ok(json!(true))
            }
            "emotion" => {
                let name = request["name"].as_str().unwrap_or("");
                let intensity = request["intensity"].as_f64().unwrap_or(1.0) as f32;
                let duration = duration(request, 3.0)?;
                self.engine.emotion(name, intensity, now + duration)?;
                Ok(json!(true))
            }
            "parameters" => {
                let values = parse_parameters(&request["values"])?;
                let duration = duration(request, 3.0)?;
                self.engine.parameters(&values, now + duration)?;
                Ok(json!(true))
            }
            "parameters_clear" => { self.engine.clear_parameters(); Ok(json!(true)) }
            "view" => {
                if let Some(zoom) = request["zoom"].as_f64() { self.zoom = (zoom as f32).clamp(0.1, 30.0); }
                if let Some(x) = request["x"].as_f64() { self.pan.x = (x as f32).clamp(-12.0, 12.0); }
                if let Some(y) = request["y"].as_f64() { self.pan.y = (y as f32).clamp(-12.0, 12.0); }
                Ok(json!(true))
            }
            "canvas" => {
                anyhow::ensure!(!self.is_recording(), "Stop recording before changing canvas size");
                let width = request["width"].as_u64().ok_or_else(|| anyhow::anyhow!("Missing width"))?;
                let height = request["height"].as_u64().ok_or_else(|| anyhow::anyhow!("Missing height"))?;
                anyhow::ensure!((16..=8192).contains(&width) && width%2==0 && (16..=8192).contains(&height) && height%2==0,
                    "Canvas dimensions must be even values from 16 to 8192");
                self.canvas = [width as u32, height as u32];
                if let Some(path) = self.model_path.clone() { self.load_model(&path)?; }
                Ok(json!(true))
            }
            "guides" => {
                if let Some(preset) = request["preset"].as_str() {
                    let selected = std::iter::once("All platforms").chain(std::iter::once("Custom"))
                        .chain(guides::PRESETS.iter().map(|(name, _)| *name))
                        .find(|name| *name == preset).ok_or_else(|| anyhow::anyhow!("Unknown guide preset"))?;
                    self.guide = selected;
                }
                if let Some(enabled) = request["enabled"].as_bool() { self.show_guides = enabled; }
                if let Some(values) = request["margins"].as_array() {
                    anyhow::ensure!(values.len()==4, "Four guide margins required");
                    for (i, value) in values.iter().enumerate() {
                        let value = value.as_f64().ok_or_else(|| anyhow::anyhow!("Guide margins must be numbers"))?;
                        anyhow::ensure!((0.0..=0.45).contains(&value), "Guide margins must be 0–0.45");
                        self.guide_margins[i] = value as f32;
                    }
                    self.guide = "Custom";
                }
                Ok(json!(self.status()["guides"]))
            }
            "load_model" => {
                let path = request["path"].as_str().ok_or_else(|| anyhow::anyhow!("Missing model path"))?;
                self.load_model(Path::new(path))?;
                Ok(json!(true))
            }
            "snapshot" => {
                let path = request["path"].as_str().ok_or_else(|| anyhow::anyhow!("Missing snapshot path"))?;
                self.renderer.as_ref().ok_or_else(|| anyhow::anyhow!("Load a model first"))?.save_png(Path::new(path))?;
                Ok(json!(true))
            }
            "ui_settings" => {
                if let Some(scale) = request["scale"].as_f64() { self.ui_scale = (scale as f32).clamp(0.75, 2.5); }
                Ok(json!(true))
            }
            "elevenlabs_refresh_voices" => { self.refresh_voices()?; Ok(json!(true)) }
            "elevenlabs_configure" => {
                if let Some(key) = request["api_key"].as_str() {
                    self.voice.set_key("elevenlabs", key, &self.data_dir, request["remember"] == true)?;
                }
                if let Some(voice) = request["settings"]["voice_id"].as_str() { self.voice.elevenlabs_voice = voice.into(); }
                if let Some(model) = request["settings"]["model"].as_str() { self.voice.elevenlabs_model = model.into(); }
                self.voice.validate()?;
                Ok(json!(true))
            }
            "voice_configure" => {
                if let Some(key) = request["api_key"].as_str() {
                    self.voice.set_key("openai", key, &self.data_dir, request["remember"] == true)?;
                }
                if let Some(voice) = request["settings"]["voice"].as_str() { self.voice.openai_voice = voice.into(); }
                if let Some(model) = request["settings"]["model"].as_str() { self.voice.openai_model = model.into(); }
                if let Some(speed) = request["settings"]["speed"].as_f64() { self.voice.speed = speed as f32; }
                if let Some(instructions) = request["settings"]["instructions"].as_str() { self.voice.instructions = instructions.into(); }
                self.voice.validate()?;
                Ok(json!(true))
            }
            "elevenlabs_forget_key" => { self.voice.forget_key("elevenlabs", &self.data_dir)?; Ok(json!(true)) }
            "voice_forget_key" => { self.voice.forget_key("openai", &self.data_dir)?; Ok(json!(true)) }
            "tts" => {
                let provider = request["provider"].as_str().unwrap_or("openai");
                let text = request["text"].as_str().ok_or_else(|| anyhow::anyhow!("Missing speech text"))?;
                let mut config = self.voice.clone();
                if let Some(selected) = request["voice"].as_str() {
                    if provider == "elevenlabs" { config.elevenlabs_voice = selected.into(); }
                    else { config.openai_voice = selected.into(); }
                }
                if let Some(model) = request["model"].as_str() {
                    if provider == "elevenlabs" { config.elevenlabs_model = model.into(); }
                    else { config.openai_model = model.into(); }
                }
                if let Some(speed) = request["speed"].as_f64() { config.speed = speed as f32; }
                if let Some(instructions) = request["instructions"].as_str() { config.instructions = instructions.into(); }
                self.start_tts(provider, text, config, request["autoplay"].as_bool().unwrap_or(true))?;
                Ok(json!({"state":"generating"}))
            }
            "audio" => {
                let path = request["path"].as_str().ok_or_else(|| anyhow::anyhow!("Missing audio path"))?;
                let path = PathBuf::from(path);
                let bytes = fs::read(&path)?;
                let clip = AudioClip::decode(bytes)?;
                self.audio_path = Some(path); self.audio_clip = Some(clip);
                Ok(json!(true))
            }
            "audio_play" => { self.play_audio()?; Ok(json!(true)) }
            "audio_stop" => { if let Some(player) = &mut self.audio_player { player.stop(); } Ok(json!(true)) }
            "record_start" => {
                let output = request["path"].as_str().or_else(|| request["output"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing recording path"))?;
                let codec = request["codec"].as_str().unwrap_or("h264");
                let fps = request["fps"].as_u64().unwrap_or(30);
                anyhow::ensure!(fps <= 120, "FPS must be 1–120");
                self.start_recording(PathBuf::from(output), fps as u32, codec)?;
                Ok(json!({"state":"recording"}))
            }
            "record_stop" => {
                self.export.as_mut().ok_or_else(|| anyhow::anyhow!("No recording started"))?.stop();
                Ok(json!({"state":"draining"}))
            }
            _ => anyhow::bail!("Command {op} has not been ported to Rust yet"),
        }
    }

    fn is_recording(&self) -> bool {
        self.export.as_ref().is_some_and(|export| matches!(export.stats().state.as_str(), "recording" | "draining" | "saving"))
    }
    fn start_recording(&mut self, output: PathBuf, fps: u32, codec: &str) -> anyhow::Result<()> {
        anyhow::ensure!(!self.is_recording(), "Recording is already running");
        let renderer = self.renderer.as_ref().ok_or_else(|| anyhow::anyhow!("Load a model first"))?;
        let audio = self.audio_path.clone();
        self.export = Some(Export::start(renderer, output, fps, codec, audio)?);
        if self.audio_clip.is_some() { self.play_audio()?; }
        self.notice = "Recording started.".into();
        Ok(())
    }

    fn refresh_voices(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.voice_busy, "Voice request already running");
        let config = self.voice.clone();
        let sender = self.voice_events.clone();
        self.voice_busy = true;
        thread::spawn(move || {
            let result = voice::list_elevenlabs_voices(&config).map_err(|error| error.to_string());
            let _ = sender.send(VoiceEvent::Voices(result));
        });
        Ok(())
    }
    fn start_tts(&mut self, provider: &str, text: &str, config: VoiceConfig, autoplay: bool) -> anyhow::Result<()> {
        anyhow::ensure!(!self.voice_busy, "Voice request already running");
        anyhow::ensure!(matches!(provider, "openai" | "elevenlabs"), "Choose OpenAI or ElevenLabs voice");
        let provider = provider.to_owned();
        let text = text.to_owned();
        let sender = self.voice_events.clone();
        let data_dir = self.data_dir.clone();
        self.voice_busy = true;
        thread::spawn(move || {
            let result = (|| -> anyhow::Result<_> {
                let (bytes, extension) = voice::synthesize(&provider, &text, &config)?;
                let clip = AudioClip::decode(bytes.clone())?;
                let root = data_dir.join("tmp");
                fs::create_dir_all(&root)?;
                let path = root.join(format!("speech-{}.{}", new_token()?, extension));
                fs::write(&path, bytes)?;
                Ok((clip, path, autoplay))
            })().map_err(|error| error.to_string());
            let _ = sender.send(VoiceEvent::Speech(result));
        });
        Ok(())
    }
    fn poll_voice(&mut self) {
        while let Ok(event) = self.voice_results.try_recv() {
            self.voice_busy = false;
            match event {
                VoiceEvent::Voices(Ok(voices)) => {
                    self.notice = format!("ElevenLabs: {} voices available.", voices.len());
                    if !voices.iter().any(|v| v.voice_id == self.voice.elevenlabs_voice) {
                        self.voice.elevenlabs_voice = voices.first().map_or(String::new(), |v| v.voice_id.clone());
                    }
                    self.voices = voices;
                }
                VoiceEvent::Speech(Ok((clip, path, autoplay))) => {
                    self.notice = format!("Speech ready ({:.1} s).", clip.duration);
                    self.audio_clip = Some(clip); self.audio_path = Some(path);
                    if autoplay { if let Err(error) = self.play_audio() { self.notice = error.to_string(); } }
                }
                VoiceEvent::Voices(Err(error)) | VoiceEvent::Speech(Err(error)) => self.notice = error,
            }
        }
    }
    fn play_audio(&mut self) -> anyhow::Result<()> {
        let clip = self.audio_clip.as_ref().ok_or_else(|| anyhow::anyhow!("Load or generate speech first"))?;
        if self.audio_player.is_none() { self.audio_player = Some(AudioPlayer::new()?); }
        self.audio_player.as_mut().unwrap().play(clip);
        Ok(())
    }

    fn process_network(&mut self) {
        let (tracking, commands) = self.inbox.drain();
        let now = self.started.elapsed().as_secs_f64();
        for (source, values) in tracking { let _ = self.engine.ingest_clean(&source, values, now); }
        for command in commands {
            let response = match self.command(&command.request) {
                Ok(result) => json!({"ok":true,"result":result}),
                Err(error) => json!({"ok":false,"error":error.to_string()}),
            };
            let _ = command.reply.send(response);
        }
    }

    fn update_model(&mut self, dt: f64) {
        let (Some(model), Some(mapper), Some(renderer)) = (self.model.as_mut(), self.mapper.as_ref(), self.renderer.as_mut()) else { return };
        let model_start = Instant::now();
        let time = self.started.elapsed().as_secs_f64();
        let mouth = self.audio_player.as_ref().and_then(AudioPlayer::position)
            .and_then(|position| self.audio_clip.as_ref().map(|clip| clip.mouth(position.as_secs_f64())));
        for (id, value) in self.engine.sample(mapper, time, dt, mouth.as_ref(), "primary") {
            model.set_parameter(&id, value);
        }
        if let Some(physics) = &mut self.physics { physics.step(model.parameters_mut(), dt as f32); }
        if let Err(error) = model.update() { self.notice = format!("Model error: {error}"); return; }
        self.model_ms = model_start.elapsed().as_secs_f32() * 1000.0;
        let render_start = Instant::now();
        renderer.set_view(self.zoom, [self.pan.x, self.pan.y]);
        if let Err(error) = renderer.render(model.canvas, &model.drawables) {
            self.notice = format!("Render error: {error}");
        }
        self.render_ms = render_start.elapsed().as_secs_f32() * 1000.0;
        if let Some(export) = &mut self.export {
            let capture_start = Instant::now();
            if let Err(error) = export.pump(renderer) { self.notice = format!("Record error: {error}"); export.stop(); }
            self.capture_ms = capture_start.elapsed().as_secs_f32() * 1000.0;
            if export.finished() {
                let status = export.stats();
                self.notice = if let Some(error) = status.error { format!("Recording failed: {error}") }
                    else { format!("Saved {} frames to {}", status.frames, status.output.display()) };
            }
        }
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.1);
        self.last_frame = now;
        ctx.set_pixels_per_point(self.ui_scale);
        if let Some(path) = self.startup_model.take() {
            if let Err(error) = self.load_model(&path) { self.notice = error.to_string(); }
        }
        self.process_network();
        self.poll_voice();
        self.update_model(dt);
        self.frame_count += 1;
        if self.fps_since.elapsed() >= Duration::from_secs(1) {
            self.fps = self.frame_count as f32 / self.fps_since.elapsed().as_secs_f32();
            self.frame_count = 0;
            self.fps_since = Instant::now();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
        egui::Panel::left("controls").default_size(310.0).resizable(true).show(root_ui, |ui| {
          egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Valkyrie Studio");
            ui.label("Rust preview · Purism Core");
            ui.separator();
            ui.label("Model manifest (.model3.json)");
            ui.text_edit_singleline(&mut self.path_field);
            if ui.button("Load model").clicked() {
                let path = PathBuf::from(self.path_field.trim());
                if let Err(error) = self.load_model(&path) { self.notice = error.to_string(); }
            }
            ui.separator();
            ui.label(format!("Mode: {}", self.engine.mode));
            ui.horizontal_wrapped(|ui| {
                for mode in ["agent", "phone", "webcam", "idle"] {
                    if ui.selectable_label(self.engine.mode == mode, mode).clicked() { let _ = self.engine.set_mode(mode); }
                }
            });
            ui.label("Framing");
            ui.add(egui::Slider::new(&mut self.zoom, 0.1..=30.0).text("Zoom"));
            ui.add(egui::Slider::new(&mut self.pan.x, -12.0..=12.0).text("Pan X"));
            ui.add(egui::Slider::new(&mut self.pan.y, -12.0..=12.0).text("Pan Y"));
            ui.separator();
            ui.label("Portrait guides");
            ui.checkbox(&mut self.show_guides, "Show safe areas");
            egui::ComboBox::from_id_salt("guide").selected_text(self.guide).show_ui(ui, |ui| {
                for name in std::iter::once("All platforms").chain(guides::PRESETS.iter().map(|(name, _)| *name)).chain(std::iter::once("Custom")) {
                    ui.selectable_value(&mut self.guide, name, name);
                }
            });
            if self.guide == "Custom" {
                for (index, label) in ["Left", "Top", "Right", "Bottom"].iter().enumerate() {
                    ui.add(egui::Slider::new(&mut self.guide_margins[index], 0.0..=0.45).text(*label));
                }
            }
            ui.separator();
            ui.add(egui::Slider::new(&mut self.ui_scale, 0.75..=2.5).text("UI scale"));
            if let Some(active) = &self.network { ui.small(format!("Agent API: {}", active.api_address)); }
            ui.label(&self.notice);
            ui.small(format!("{:.1} FPS · model {:.1} ms · render {:.1} ms · capture {:.1} ms", self.fps, self.model_ms, self.render_ms, self.capture_ms));
            ui.small("F11 toggles fullscreen. No model files are bundled.");
            ui.separator();
            ui.heading("Voice");
            ui.horizontal(|ui| {
                for provider in ["elevenlabs", "openai"] {
                    let label = if provider == "elevenlabs" { "ElevenLabs" } else { "OpenAI" };
                    if ui.selectable_label(self.voice_provider == provider, label).clicked() { self.voice_provider = provider; }
                }
            });
            if self.voice_provider == "elevenlabs" {
                ui.label(if self.voice.elevenlabs_key.is_some() { "ElevenLabs key available" } else { "Add an ElevenLabs key" });
                if ui.button("Refresh my voices").clicked() {
                    if let Err(error) = self.refresh_voices() { self.notice = error.to_string(); }
                }
                let current = self.voices.iter().find(|v| v.voice_id == self.voice.elevenlabs_voice)
                    .map_or("Select a voice", |v| v.name.as_str());
                egui::ComboBox::from_id_salt("eleven-voices").selected_text(current).show_ui(ui, |ui| {
                    for choice in &self.voices {
                        ui.selectable_value(&mut self.voice.elevenlabs_voice, choice.voice_id.clone(), &choice.name);
                    }
                });
                ui.text_edit_singleline(&mut self.voice.elevenlabs_model);
            } else {
                ui.label(if self.voice.openai_key.is_some() { "OpenAI key available" } else { "Add an OpenAI key" });
                egui::ComboBox::from_id_salt("openai-voices").selected_text(&self.voice.openai_voice).show_ui(ui, |ui| {
                    for choice in voice::OPENAI_VOICES { ui.selectable_value(&mut self.voice.openai_voice, (*choice).into(), *choice); }
                });
                ui.add(egui::Slider::new(&mut self.voice.speed, 0.25..=4.0).text("Speed"));
            }
            ui.add(egui::TextEdit::singleline(&mut self.key_field).password(true).hint_text("API key (kept off screen)"));
            ui.checkbox(&mut self.remember_key, "Remember on this Windows account");
            ui.horizontal(|ui| {
                if ui.button("Use key").clicked() {
                    let provider = self.voice_provider;
                    match self.voice.set_key(provider, &self.key_field, &self.data_dir, self.remember_key) {
                        Ok(()) => { self.key_field.clear(); self.notice = "Voice key ready.".into(); }
                        Err(error) => self.notice = error.to_string(),
                    }
                }
                if ui.button("Forget key").clicked() {
                    if let Err(error) = self.voice.forget_key(self.voice_provider, &self.data_dir) { self.notice = error.to_string(); }
                }
            });
            ui.label("Typed dialogue");
            ui.add(egui::TextEdit::multiline(&mut self.speech_text).desired_rows(4));
            if ui.add_enabled(!self.voice_busy, egui::Button::new(if self.voice_busy { "Working…" } else { "Speak and animate" })).clicked() {
                let provider = self.voice_provider;
                let text = self.speech_text.clone();
                let config = self.voice.clone();
                if let Err(error) = self.start_tts(provider, &text, config, true) { self.notice = error.to_string(); }
            }
            ui.horizontal(|ui| {
                if ui.button("Replay").clicked() { if let Err(error)=self.play_audio() { self.notice=error.to_string(); } }
                if ui.button("Stop").clicked() { if let Some(player)=&mut self.audio_player { player.stop(); } }
            });
            ui.separator();
            ui.heading("Record video");
            ui.label("Output path (.mp4, .mov, or .webm)");
            ui.text_edit_singleline(&mut self.record_output);
            egui::ComboBox::from_id_salt("record-codec").selected_text(self.record_codec).show_ui(ui, |ui| {
                for codec in ["h264", "h265", "prores", "vp9"] {
                    ui.selectable_value(&mut self.record_codec, codec, codec);
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.is_recording(), egui::Button::new("Start recording")).clicked() {
                    let path = PathBuf::from(self.record_output.trim());
                    if let Err(error) = self.start_recording(path, 30, self.record_codec) { self.notice = error.to_string(); }
                }
                if ui.add_enabled(self.is_recording(), egui::Button::new("Stop recording")).clicked() {
                    if let Some(export) = &mut self.export { export.stop(); }
                }
            });
            if let Some(export) = &self.export { let stats = export.stats(); ui.small(format!("{} · {} frames · {} dropped", stats.state, stats.frames, stats.dropped)); }
          });
        });
        egui::CentralPanel::default().show(root_ui, |ui| {
            let bounds = ui.available_rect_before_wrap();
            let aspect = self.canvas[0] as f32 / self.canvas[1] as f32;
            let width = (bounds.height() * aspect).min(bounds.width());
            let portrait = egui::Rect::from_center_size(bounds.center(), egui::vec2(width, width / aspect));
            ui.painter().rect_filled(portrait, 0.0, egui::Color32::from_rgb(24, 24, 32));
            if let Some(renderer) = &self.renderer {
                let rect = renderer.image.rect(portrait, 1.0);
                ui.painter().image(renderer.image.id, rect, egui::Rect::from_min_max(
                    egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            } else {
                ui.painter().text(portrait.center(), egui::Align2::CENTER_CENTER,
                    "Load a local model to preview it", egui::FontId::proportional(22.0), egui::Color32::LIGHT_GRAY);
            }
            if self.show_guides && aspect < 0.8 { draw_guide(ui.painter(), portrait, self.guide, self.guide_margins); }
        });
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn duration(request: &Value, default: f64) -> anyhow::Result<f64> {
    let duration = request["duration"].as_f64().unwrap_or(default);
    anyhow::ensure!((0.0..=600.0).contains(&duration), "Duration must be 0–600 seconds");
    Ok(duration)
}

fn parse_parameters(value: &Value) -> anyhow::Result<Values> {
    let raw = value.as_object().ok_or_else(|| anyhow::anyhow!("parameters values must be an object"))?;
    anyhow::ensure!(raw.len() <= 512, "Too many parameter IDs");
    raw.iter().map(|(id, value)| {
        let number = value.as_f64().ok_or_else(|| anyhow::anyhow!("Parameter {id} must be numeric"))?;
        anyhow::ensure!(number.is_finite(), "Parameter {id} is not finite");
        Ok((id.clone(), number as f32))
    }).collect()
}

fn draw_guide(painter: &egui::Painter, portrait: egui::Rect, platform: &str, custom: [f32; 4]) {
    let [left, top, right, bottom] = guides::margins(platform, custom).unwrap_or(custom);
    let safe = egui::Rect::from_min_max(
        egui::pos2(portrait.left() + portrait.width() * left, portrait.top() + portrait.height() * top),
        egui::pos2(portrait.right() - portrait.width() * right, portrait.bottom() - portrait.height() * bottom));
    let shade = egui::Color32::from_black_alpha(75);
    for region in [
        egui::Rect::from_min_max(portrait.min, egui::pos2(portrait.right(), safe.top())),
        egui::Rect::from_min_max(egui::pos2(portrait.left(), safe.bottom()), portrait.max),
        egui::Rect::from_min_max(egui::pos2(portrait.left(), safe.top()), egui::pos2(safe.left(), safe.bottom())),
        egui::Rect::from_min_max(egui::pos2(safe.right(), safe.top()), egui::pos2(portrait.right(), safe.bottom())),
    ] { painter.rect_filled(region, 0.0, shade); }
    painter.rect_stroke(safe, 0.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 240, 220)), egui::StrokeKind::Inside);
    painter.text(safe.left_top() + egui::vec2(4.0, 4.0), egui::Align2::LEFT_TOP,
        platform, egui::FontId::proportional(14.0), egui::Color32::from_rgb(0, 240, 220));
}
