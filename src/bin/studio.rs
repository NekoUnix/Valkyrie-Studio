#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{egui, egui_wgpu::RenderState};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};
use valkyrie_studio::{
    app_paths,
    audio::{AudioClip, AudioPlayer},
    engine::{Engine, EngineConfig},
    export::Export,
    guides,
    model::ModelAssets,
    network::{Inbox, Network, NetworkConfig, new_token},
    physics::{GroupSettings, MotionStyle, Physics, PhysicsSettings},
    purism::CubismModel,
    renderer::ModelRenderer,
    tracking::{Mapper, Parameter, Values},
    voice::{self, VoiceChoice, VoiceConfig},
};

#[path = "studio/studio_ui.rs"]
mod studio_ui;
use studio_ui::configure_theme;

enum VoiceEvent {
    Voices(Result<Vec<VoiceChoice>, String>),
    Speech(Result<(AudioClip, PathBuf, bool), String>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StudioTab {
    Stage,
    Physics,
    Voice,
    Capture,
    Inputs,
}

fn main() -> eframe::Result {
    let mut args = env::args_os().skip(1);
    let mut startup_model = None;
    while let Some(arg) = args.next() {
        if arg == "--model" {
            startup_model = args.next().map(PathBuf::from);
        } else if arg == "--help" {
            println!("Usage: valkyrie-studio [--model PATH_TO_MODEL3_JSON]");
            return Ok(());
        }
    }
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if cfg!(windows)
        && let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup
    {
        setup.instance_descriptor.backends =
            wgpu::Backends::from_env().unwrap_or(wgpu::Backends::DX12);
    }
    let icon = image::load_from_memory(include_bytes!("../../assets/valkyrie-icon-256.png"))
        .expect("bundled app icon")
        .thumbnail(256, 256)
        .to_rgba8();
    let icon = egui::IconData {
        rgba: icon.as_raw().clone(),
        width: icon.width(),
        height: icon.height(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Valkyrie Studio — Alpha")
            .with_app_id("com.nekounix.valkyrie-studio")
            .with_icon(icon)
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 600.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native(
        "Valkyrie Studio",
        options,
        Box::new(move |cc| Ok(Box::new(Studio::new(cc, startup_model.take())))),
    )
}

struct Studio {
    render_state: Option<RenderState>,
    model: Option<CubismModel>,
    renderer: Option<ModelRenderer>,
    physics: Option<Physics>,
    physics_settings: PhysicsSettings,
    physics_defaults: PhysicsSettings,
    physics_profile_id: Option<String>,
    physics_search: String,
    physics_modified_only: bool,
    physics_dirty: Option<Instant>,
    mapper: Option<Mapper>,
    engine: Engine,
    inbox: Inbox,
    network: Option<Network>,
    network_config: NetworkConfig,
    api_token: Option<String>,
    tracking_bind_field: String,
    tracking_ports_field: String,
    model_path: Option<PathBuf>,
    data_dir: PathBuf,
    path_field: String,
    recent_models: Vec<PathBuf>,
    startup_model: Option<PathBuf>,
    notice: String,
    zoom: f32,
    pan: egui::Vec2,
    ui_scale: f32,
    selected_tab: StudioTab,
    guide: &'static str,
    show_guides: bool,
    mock_ui: bool,
    guide_opacity: f32,
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
        configure_theme(&cc.egui_ctx);
        let inbox = Inbox::new();
        let mut notice = "Load a local .model3.json to begin.".to_owned();
        let data_dir = app_paths::data_dir().unwrap_or_else(|error| {
            notice = format!("User data path error: {error}");
            env::temp_dir().join("ValkyrieStudio")
        });
        let mut network = None;
        let mut network_config = NetworkConfig::default();
        let token = env::var("L2D_API_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| new_token().ok());
        if let Some(ref token) = token {
            if let Ok(port) = env::var("L2D_API_PORT") {
                if let Ok(port) = port.parse() {
                    network_config.api.set_port(port);
                }
            }
            match Network::start(network_config.clone(), &inbox, token.clone()) {
                Ok(active) => {
                    let file = data_dir
                        .join("tmp")
                        .join(if active.api_address.port() == 4141 {
                            "api-token".into()
                        } else {
                            format!("api-token-{}", active.api_address.port())
                        });
                    if fs::create_dir_all(file.parent().unwrap())
                        .and_then(|_| fs::write(&file, token))
                        .is_ok()
                    {
                        network = Some(active);
                    } else {
                        notice = "Could not write the local agent token; API is disabled.".into();
                    }
                }
                Err(error) => notice = format!("Agent API unavailable: {error}"),
            }
        }
        let recent_models: Vec<PathBuf> = fs::read(data_dir.join("recent-models.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let startup_model =
            startup_model.or_else(|| recent_models.iter().find(|p| p.is_file()).cloned());
        let (voice_events, voice_results) = mpsc::channel();
        let voice = VoiceConfig::load_saved(&data_dir);
        Self {
            render_state: cc.wgpu_render_state.clone(),
            model: None,
            renderer: None,
            physics: None,
            physics_settings: PhysicsSettings::default(),
            physics_defaults: PhysicsSettings::default(),
            physics_profile_id: None,
            physics_search: String::new(),
            physics_modified_only: false,
            physics_dirty: None,
            mapper: None,
            engine: Engine::new(EngineConfig::default()),
            inbox,
            network,
            network_config,
            api_token: token,
            tracking_bind_field: "127.0.0.1".into(),
            tracking_ports_field: "15483,8001,49983".into(),
            model_path: None,
            data_dir,
            path_field: startup_model
                .as_ref()
                .map_or(String::new(), |p| p.display().to_string()),
            startup_model,
            recent_models,
            notice,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            ui_scale: 1.0,
            selected_tab: StudioTab::Stage,
            guide: "All platforms",
            show_guides: true,
            mock_ui: true,
            guide_opacity: 0.30,
            guide_margins: [0.06, 0.12, 0.2, 0.25],
            canvas: [1080, 1920],
            last_frame: Instant::now(),
            started: Instant::now(),
            command_count: 0,
            voice,
            voices: Vec::new(),
            voice_events,
            voice_results,
            voice_busy: false,
            speech_text: String::new(),
            voice_provider: "elevenlabs",
            key_field: String::new(),
            remember_key: cfg!(windows),
            audio_player: None,
            audio_clip: None,
            audio_path: None,
            export: None,
            record_output: String::new(),
            record_codec: "h264",
            frame_count: 0,
            fps_since: Instant::now(),
            fps: 0.0,
            model_ms: 0.0,
            render_ms: 0.0,
            capture_ms: 0.0,
        }
    }

    fn load_model(&mut self, path: &Path) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.is_recording(),
            "Stop recording before changing models"
        );
        if self.physics_dirty.is_some() {
            self.save_physics_settings()?;
        }
        let assets = ModelAssets::open(path)?;
        let profile_id = format!(
            "{:016x}",
            assets
                .moc
                .iter()
                .fold(0xcbf29ce484222325_u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                })
        );
        let mut model = CubismModel::load(Path::new(""), &assets.moc, assets.textures.len())?;
        model.update()?;
        let state = self
            .render_state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WGPU is unavailable"))?;
        let mut renderer = ModelRenderer::new_sized(
            state,
            model.canvas,
            &model.drawables,
            &assets.textures,
            self.canvas[0],
            self.canvas[1],
        )?;
        renderer.render(model.canvas, &model.drawables)?;
        let mapper = Mapper::new(model.parameters().iter().map(|p| Parameter {
            id: p.id.clone(),
            min: p.min,
            max: p.max,
            default: p.default,
        }));
        let mut physics = assets
            .physics
            .as_ref()
            .map(|definition| Physics::load(definition, model.parameters()))
            .transpose()?;
        let mut physics_settings = PhysicsSettings::default();
        if let Some(active) = &mut physics {
            if let Some(vtube) = &assets.vtube {
                let imported = &vtube["PhysicsSettings"];
                physics_settings.enabled = imported["Use"].as_bool().unwrap_or(true);
                physics_settings.strength =
                    (imported["PhysicsStrength"].as_f64().unwrap_or(50.0) as f32 / 50.0)
                        .clamp(0.0, 2.0);
                physics_settings.wind = (imported["WindStrength"].as_f64().unwrap_or(0.0) as f32
                    / 100.0)
                    .clamp(-1.0, 1.0);
                let mut multipliers = BTreeMap::new();
                if let Some(rows) =
                    vtube["PhysicsCustomizationSettings"]["PhysicsMultipliersPerPhysicsGroup"]
                        .as_array()
                {
                    for row in rows.iter().take(256) {
                        if let (Some(id), Some(value)) = (row["ID"].as_str(), row["Value"].as_f64())
                        {
                            if value.is_finite() && id.len() <= 256 {
                                multipliers.insert(id.to_owned(), (value as f32).clamp(0.0, 5.0));
                            }
                        }
                    }
                }
                active.set_multipliers(&multipliers);
                if let Some(rows) =
                    vtube["PhysicsCustomizationSettings"]["WindMultipliersPerPhysicsGroup"]
                        .as_array()
                {
                    for row in rows.iter().take(256) {
                        if let (Some(id), Some(value)) = (row["ID"].as_str(), row["Value"].as_f64())
                        {
                            if value.is_finite() && id.len() <= 256 {
                                physics_settings
                                    .groups
                                    .entry(id.to_owned())
                                    .or_default()
                                    .wind =
                                    (physics_settings.wind * (value as f32 - 1.0)).clamp(-1.0, 1.0);
                            }
                        }
                    }
                }
            }
            // New models use a calmer baseline for tail-named chains. Existing
            // saved groups retain their user's tuning.
            for group in active.groups() {
                if group.name.to_ascii_lowercase().contains("tail") {
                    physics_settings
                        .groups
                        .entry(group.id)
                        .or_insert(GroupSettings {
                            inertia: 0.55,
                            ..Default::default()
                        });
                }
            }
            let physics_defaults = physics_settings.clone();
            let profile = self
                .data_dir
                .join("physics-profiles")
                .join(format!("{profile_id}.json"));
            if let Ok(bytes) = fs::read(profile) {
                if let Ok(mut saved) = serde_json::from_slice::<PhysicsSettings>(&bytes) {
                    if saved.validate().is_ok() {
                        for (id, group) in &physics_defaults.groups {
                            saved.groups.entry(id.clone()).or_insert(*group);
                        }
                        physics_settings = saved;
                    }
                }
            }
            active.configure(&physics_settings);
            self.physics_defaults = physics_defaults;
        } else {
            self.physics_defaults = PhysicsSettings::default();
        }
        self.notice = format!(
            "Loaded {} with {} parameters and {} atlases.",
            assets
                .manifest
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            model.parameters().len(),
            assets.textures.len()
        );
        self.model_path = Some(assets.manifest.clone());
        self.renderer = Some(renderer);
        self.physics = physics;
        self.physics_settings = physics_settings;
        self.physics_dirty = None;
        self.physics_profile_id = Some(profile_id);
        self.recent_models.retain(|p| p != &assets.manifest);
        self.recent_models.insert(0, assets.manifest);
        self.recent_models.truncate(8);
        let _ = fs::create_dir_all(&self.data_dir);
        let _ = fs::write(
            self.data_dir.join("recent-models.json"),
            serde_json::to_vec_pretty(&self.recent_models)?,
        );
        self.mapper = Some(mapper);
        self.model = Some(model);
        self.engine = Engine::new(EngineConfig::default());
        Ok(())
    }

    fn save_physics_settings(&mut self) -> anyhow::Result<()> {
        self.physics_settings.validate()?;
        let id = self
            .physics_profile_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Load a model first"))?;
        let dir = self.data_dir.join("physics-profiles");
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&self.physics_settings)?,
        )?;
        if let Some(physics) = &mut self.physics {
            physics.configure(&self.physics_settings);
        }
        self.physics_dirty = None;
        Ok(())
    }

    fn change_canvas(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.is_recording(),
            "Stop recording before changing canvas size"
        );
        anyhow::ensure!(
            (16..=8192).contains(&width)
                && width % 2 == 0
                && (16..=8192).contains(&height)
                && height % 2 == 0,
            "Canvas dimensions must be even values from 16 to 8192"
        );
        let previous = self.canvas;
        self.canvas = [width, height];
        if let Some(path) = self.model_path.clone() {
            if let Err(error) = self.load_model(&path) {
                self.canvas = previous;
                return Err(error);
            }
        }
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
                "mock_ui": self.mock_ui, "opacity": self.guide_opacity,
                "margins": guides::margins(self.guide, self.guide_margins)},
            "notice": self.notice,
            "voice": self.voice.public_status(),
            "voice_busy": self.voice_busy,
            "physics": self.physics.as_ref().map(|p| json!({"settings": self.physics_settings, "group_count": p.group_count()})),
            "audio_path": self.audio_path.as_ref().map(|p| p.display().to_string()),
            "audio_duration": self.audio_clip.as_ref().map(|clip| clip.duration),
            "recording": self.export.as_ref().map(Export::stats),
            "runtime": "Rust + Purism Core"
            ,"tracking_listeners": self.network.as_ref().map(|n| &n.tracking_addresses)
            ,"performance": {"fps": self.fps, "model_ms": self.model_ms, "render_ms": self.render_ms,
                "capture_ms": self.capture_ms}
        })
    }

    fn restart_tracking(&mut self) -> anyhow::Result<()> {
        let bind: IpAddr = self.tracking_bind_field.trim().parse()?;
        let ports: Vec<u16> = self
            .tracking_ports_field
            .split(',')
            .map(|s| s.trim().parse())
            .collect::<Result<_, _>>()?;
        anyhow::ensure!(
            !ports.is_empty() && ports.len() <= 8 && ports.iter().all(|p| *p >= 1024),
            "Enter one to eight UDP ports, each at least 1024"
        );
        anyhow::ensure!(
            ports.iter().collect::<std::collections::HashSet<_>>().len() == ports.len(),
            "Tracking ports must be unique"
        );
        let mut next = self.network_config.clone();
        next.tracking_bind = bind;
        next.tracking_ports = ports;
        let token = self
            .api_token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Agent API token is unavailable"))?
            .clone();
        self.network.take();
        match Network::start(next.clone(), &self.inbox, token.clone()) {
            Ok(active) => {
                self.network = Some(active);
                self.network_config = next;
                self.notice = format!(
                    "Tracking UDP listening on {}",
                    self.network
                        .as_ref()
                        .unwrap()
                        .tracking_addresses
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                Ok(())
            }
            Err(error) => {
                self.network = Network::start(self.network_config.clone(), &self.inbox, token).ok();
                anyhow::bail!("Could not start tracking listener: {error}")
            }
        }
    }

    fn command(&mut self, request: &Value) -> anyhow::Result<Value> {
        let op = request["op"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing op"))?;
        let now = self.started.elapsed().as_secs_f64();
        self.command_count += 1;
        match op {
            "status" => Ok(self.status()),
            "schema" => {
                let model = self
                    .model
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Load a model first"))?;
                Ok(
                    json!({"parameters": model.parameters(), "drawables": model.drawables.iter().map(|d| &d.id).collect::<Vec<_>>(), "canvas": model.canvas}),
                )
            }
            "physics" => {
                anyhow::ensure!(self.physics.is_some(), "Load a model with physics first");
                if request["settle"] == true {
                    self.physics.as_mut().unwrap().reset();
                }
                if !request["settings"].is_null() {
                    let settings: PhysicsSettings =
                        serde_json::from_value(request["settings"].clone())?;
                    settings.validate()?;
                    self.physics_settings = settings;
                    self.save_physics_settings()?;
                }
                Ok(
                    json!({"settings": self.physics_settings, "groups": self.physics.as_ref().unwrap().groups()}),
                )
            }
            "mode" => {
                self.engine
                    .set_mode(request["mode"].as_str().unwrap_or(""))?;
                Ok(json!(true))
            }
            "calibrate" => {
                self.engine.calibration.reset();
                Ok(json!(true))
            }
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
            "parameters_clear" => {
                self.engine.clear_parameters();
                Ok(json!(true))
            }
            "view" => {
                if let Some(zoom) = request["zoom"].as_f64() {
                    self.zoom = (zoom as f32).clamp(0.1, 30.0);
                }
                if let Some(x) = request["x"].as_f64() {
                    self.pan.x = (x as f32).clamp(-12.0, 12.0);
                }
                if let Some(y) = request["y"].as_f64() {
                    self.pan.y = (y as f32).clamp(-12.0, 12.0);
                }
                Ok(json!(true))
            }
            "canvas" => {
                let width = request["width"]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("Missing width"))?;
                let height = request["height"]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("Missing height"))?;
                anyhow::ensure!(
                    width <= 8192 && height <= 8192,
                    "Canvas dimensions must be at most 8192"
                );
                self.change_canvas(width as u32, height as u32)?;
                Ok(json!(true))
            }
            "guides" => {
                if let Some(preset) = request["preset"].as_str() {
                    let selected = std::iter::once("All platforms")
                        .chain(std::iter::once("Custom"))
                        .chain(guides::PRESETS.iter().map(|(name, _)| *name))
                        .find(|name| *name == preset)
                        .ok_or_else(|| anyhow::anyhow!("Unknown guide preset"))?;
                    self.guide = selected;
                }
                if let Some(enabled) = request["enabled"].as_bool() {
                    self.show_guides = enabled;
                }
                if let Some(mock_ui) = request["mock_ui"].as_bool() {
                    self.mock_ui = mock_ui;
                }
                if let Some(opacity) = request["opacity"].as_f64() {
                    anyhow::ensure!(
                        (0.0..=0.8).contains(&opacity),
                        "Guide opacity must be 0–0.8"
                    );
                    self.guide_opacity = opacity as f32;
                }
                if let Some(values) = request["margins"].as_array() {
                    anyhow::ensure!(values.len() == 4, "Four guide margins required");
                    for (i, value) in values.iter().enumerate() {
                        let value = value
                            .as_f64()
                            .ok_or_else(|| anyhow::anyhow!("Guide margins must be numbers"))?;
                        anyhow::ensure!(
                            (0.0..=0.45).contains(&value),
                            "Guide margins must be 0–0.45"
                        );
                        self.guide_margins[i] = value as f32;
                    }
                    self.guide = "Custom";
                }
                Ok(json!(self.status()["guides"]))
            }
            "load_model" => {
                let path = request["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing model path"))?;
                self.load_model(Path::new(path))?;
                Ok(json!(true))
            }
            "snapshot" => {
                let path = request["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing snapshot path"))?;
                self.renderer
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Load a model first"))?
                    .save_png(Path::new(path))?;
                Ok(json!(true))
            }
            "ui_settings" => {
                if let Some(scale) = request["scale"].as_f64() {
                    self.ui_scale = (scale as f32).clamp(0.75, 2.5);
                }
                Ok(json!(true))
            }
            "elevenlabs_refresh_voices" => {
                self.refresh_voices()?;
                Ok(json!(true))
            }
            "elevenlabs_configure" => {
                if let Some(key) = request["api_key"].as_str() {
                    self.voice.set_key(
                        "elevenlabs",
                        key,
                        &self.data_dir,
                        request["remember"] == true,
                    )?;
                }
                if let Some(voice) = request["settings"]["voice_id"].as_str() {
                    self.voice.elevenlabs_voice = voice.into();
                }
                if let Some(model) = request["settings"]["model"].as_str() {
                    self.voice.elevenlabs_model = model.into();
                }
                self.voice.validate()?;
                Ok(json!(true))
            }
            "voice_configure" => {
                if let Some(key) = request["api_key"].as_str() {
                    self.voice.set_key(
                        "openai",
                        key,
                        &self.data_dir,
                        request["remember"] == true,
                    )?;
                }
                if let Some(voice) = request["settings"]["voice"].as_str() {
                    self.voice.openai_voice = voice.into();
                }
                if let Some(model) = request["settings"]["model"].as_str() {
                    self.voice.openai_model = model.into();
                }
                if let Some(speed) = request["settings"]["speed"].as_f64() {
                    self.voice.speed = speed as f32;
                }
                if let Some(instructions) = request["settings"]["instructions"].as_str() {
                    self.voice.instructions = instructions.into();
                }
                self.voice.validate()?;
                Ok(json!(true))
            }
            "elevenlabs_forget_key" => {
                self.voice.forget_key("elevenlabs", &self.data_dir)?;
                Ok(json!(true))
            }
            "voice_forget_key" => {
                self.voice.forget_key("openai", &self.data_dir)?;
                Ok(json!(true))
            }
            "tts" => {
                let provider = request["provider"].as_str().unwrap_or("openai");
                let text = request["text"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing speech text"))?;
                let mut config = self.voice.clone();
                if let Some(selected) = request["voice"].as_str() {
                    if provider == "elevenlabs" {
                        config.elevenlabs_voice = selected.into();
                    } else {
                        config.openai_voice = selected.into();
                    }
                }
                if let Some(model) = request["model"].as_str() {
                    if provider == "elevenlabs" {
                        config.elevenlabs_model = model.into();
                    } else {
                        config.openai_model = model.into();
                    }
                }
                if let Some(speed) = request["speed"].as_f64() {
                    config.speed = speed as f32;
                }
                if let Some(instructions) = request["instructions"].as_str() {
                    config.instructions = instructions.into();
                }
                self.start_tts(
                    provider,
                    text,
                    config,
                    request["autoplay"].as_bool().unwrap_or(true),
                )?;
                Ok(json!({"state":"generating"}))
            }
            "audio" => {
                let path = request["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing audio path"))?;
                let path = PathBuf::from(path);
                let bytes = fs::read(&path)?;
                let clip = AudioClip::decode(bytes)?;
                self.audio_path = Some(path);
                self.audio_clip = Some(clip);
                Ok(json!(true))
            }
            "audio_play" => {
                self.play_audio()?;
                Ok(json!(true))
            }
            "audio_stop" => {
                if let Some(player) = &mut self.audio_player {
                    player.stop();
                }
                Ok(json!(true))
            }
            "record_start" => {
                let output = request["path"]
                    .as_str()
                    .or_else(|| request["output"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing recording path"))?;
                let codec = request["codec"].as_str().unwrap_or("h264");
                let fps = request["fps"].as_u64().unwrap_or(30);
                anyhow::ensure!(fps <= 120, "FPS must be 1–120");
                self.start_recording(PathBuf::from(output), fps as u32, codec)?;
                Ok(json!({"state":"recording"}))
            }
            "record_stop" => {
                self.export
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("No recording started"))?
                    .stop();
                Ok(json!({"state":"draining"}))
            }
            _ => anyhow::bail!("Command {op} has not been ported to Rust yet"),
        }
    }

    fn is_recording(&self) -> bool {
        self.export.as_ref().is_some_and(|export| {
            matches!(
                export.stats().state.as_str(),
                "recording" | "draining" | "saving"
            )
        })
    }
    fn start_recording(&mut self, output: PathBuf, fps: u32, codec: &str) -> anyhow::Result<()> {
        anyhow::ensure!(!self.is_recording(), "Recording is already running");
        let renderer = self
            .renderer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Load a model first"))?;
        let audio = self.audio_path.clone();
        self.export = Some(Export::start(renderer, output, fps, codec, audio)?);
        if self.audio_clip.is_some() {
            self.play_audio()?;
        }
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
    fn start_tts(
        &mut self,
        provider: &str,
        text: &str,
        config: VoiceConfig,
        autoplay: bool,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!self.voice_busy, "Voice request already running");
        anyhow::ensure!(
            matches!(provider, "openai" | "elevenlabs"),
            "Choose OpenAI or ElevenLabs voice"
        );
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
            })()
            .map_err(|error| error.to_string());
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
                    if !voices
                        .iter()
                        .any(|v| v.voice_id == self.voice.elevenlabs_voice)
                    {
                        self.voice.elevenlabs_voice =
                            voices.first().map_or(String::new(), |v| v.voice_id.clone());
                    }
                    self.voices = voices;
                }
                VoiceEvent::Speech(Ok((clip, path, autoplay))) => {
                    self.notice = format!("Speech ready ({:.1} s).", clip.duration);
                    self.audio_clip = Some(clip);
                    self.audio_path = Some(path);
                    if autoplay {
                        if let Err(error) = self.play_audio() {
                            self.notice = error.to_string();
                        }
                    }
                }
                VoiceEvent::Voices(Err(error)) | VoiceEvent::Speech(Err(error)) => {
                    self.notice = error
                }
            }
        }
    }
    fn play_audio(&mut self) -> anyhow::Result<()> {
        let clip = self
            .audio_clip
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Load or generate speech first"))?;
        if self.audio_player.is_none() {
            self.audio_player = Some(AudioPlayer::new()?);
        }
        self.audio_player.as_mut().unwrap().play(clip);
        Ok(())
    }

    fn process_network(&mut self) {
        let (tracking, commands) = self.inbox.drain();
        let now = self.started.elapsed().as_secs_f64();
        for (source, values) in tracking {
            let _ = self.engine.ingest_clean(&source, values, now);
        }
        for command in commands {
            let response = match self.command(&command.request) {
                Ok(result) => json!({"ok":true,"result":result}),
                Err(error) => json!({"ok":false,"error":error.to_string()}),
            };
            let _ = command.reply.send(response);
        }
    }

    fn update_model(&mut self, dt: f64, physics_dt: f64) {
        let (Some(model), Some(mapper), Some(renderer)) = (
            self.model.as_mut(),
            self.mapper.as_ref(),
            self.renderer.as_mut(),
        ) else {
            return;
        };
        let model_start = Instant::now();
        let time = self.started.elapsed().as_secs_f64();
        let mouth = self
            .audio_player
            .as_ref()
            .and_then(AudioPlayer::position)
            .and_then(|position| {
                self.audio_clip
                    .as_ref()
                    .map(|clip| clip.mouth(position.as_secs_f64()))
            });
        for (id, value) in self
            .engine
            .sample(mapper, time, dt, mouth.as_ref(), "primary")
        {
            model.set_parameter(&id, value);
        }
        if let Some(physics) = &mut self.physics {
            physics.step(model.parameters_mut(), physics_dt.min(1.0) as f32);
        }
        if let Err(error) = model.update() {
            self.notice = format!("Model error: {error}");
            return;
        }
        self.model_ms = model_start.elapsed().as_secs_f32() * 1000.0;
        let render_start = Instant::now();
        renderer.set_view(self.zoom, [self.pan.x, self.pan.y]);
        if let Err(error) = renderer.render(model.canvas, &model.drawables) {
            self.notice = format!("Render error: {error}");
        }
        self.render_ms = render_start.elapsed().as_secs_f32() * 1000.0;
        if let Some(export) = &mut self.export {
            let capture_start = Instant::now();
            if let Err(error) = export.pump(renderer) {
                self.notice = format!("Record error: {error}");
                export.stop();
            }
            self.capture_ms = capture_start.elapsed().as_secs_f32() * 1000.0;
            if export.finished() {
                let status = export.stats();
                self.notice = if let Some(error) = status.error {
                    format!("Recording failed: {error}")
                } else {
                    format!(
                        "Saved {} frames to {}",
                        status.frames,
                        status.output.display()
                    )
                };
            }
        }
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_ui(root_ui);
    }

    fn on_exit(&mut self) {
        if self.physics_dirty.is_some() {
            let _ = self.save_physics_settings();
        }
    }
}

fn duration(request: &Value, default: f64) -> anyhow::Result<f64> {
    let duration = request["duration"].as_f64().unwrap_or(default);
    anyhow::ensure!(
        (0.0..=600.0).contains(&duration),
        "Duration must be 0–600 seconds"
    );
    Ok(duration)
}

fn parse_parameters(value: &Value) -> anyhow::Result<Values> {
    let raw = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("parameters values must be an object"))?;
    anyhow::ensure!(raw.len() <= 512, "Too many parameter IDs");
    raw.iter()
        .map(|(id, value)| {
            let number = value
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("Parameter {id} must be numeric"))?;
            anyhow::ensure!(number.is_finite(), "Parameter {id} is not finite");
            Ok((id.clone(), number as f32))
        })
        .collect()
}

fn draw_guide(
    painter: &egui::Painter,
    portrait: egui::Rect,
    platform: &str,
    custom: [f32; 4],
    mock_ui: bool,
    opacity: f32,
) {
    let painter = painter.with_clip_rect(portrait);
    let [left, top, right, bottom] = guides::margins(platform, custom).unwrap_or(custom);
    let safe = egui::Rect::from_min_max(
        egui::pos2(
            portrait.left() + portrait.width() * left,
            portrait.top() + portrait.height() * top,
        ),
        egui::pos2(
            portrait.right() - portrait.width() * right,
            portrait.bottom() - portrait.height() * bottom,
        ),
    );
    let shade = egui::Color32::from_rgba_unmultiplied(13, 10, 28, (opacity * 255.0) as u8);
    for region in [
        egui::Rect::from_min_max(portrait.min, egui::pos2(portrait.right(), safe.top())),
        egui::Rect::from_min_max(egui::pos2(portrait.left(), safe.bottom()), portrait.max),
        egui::Rect::from_min_max(
            egui::pos2(portrait.left(), safe.top()),
            egui::pos2(safe.left(), safe.bottom()),
        ),
        egui::Rect::from_min_max(
            egui::pos2(safe.right(), safe.top()),
            egui::pos2(portrait.right(), safe.bottom()),
        ),
    ] {
        painter.rect_filled(region, 0.0, shade);
    }
    painter.rect_stroke(
        safe,
        0.0,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(104, 231, 224)),
        egui::StrokeKind::Inside,
    );
    let point = |x: f32, y: f32| {
        egui::pos2(
            portrait.left() + portrait.width() * x,
            portrait.top() + portrait.height() * y,
        )
    };
    let scale = (portrait.width() / 390.0).clamp(0.65, 1.3);
    painter.rect_filled(
        egui::Rect::from_min_max(point(0.0, 0.0), point(1.0, 0.07)),
        0.0,
        egui::Color32::from_black_alpha(190),
    );
    painter.text(
        point(0.035, 0.035),
        egui::Align2::LEFT_CENTER,
        format!("{}  ·  PREVIEW", platform),
        egui::FontId::proportional(12.0 * scale),
        egui::Color32::from_rgb(218, 248, 246),
    );
    painter.text(
        safe.left_top() + egui::vec2(5.0, 5.0),
        egui::Align2::LEFT_TOP,
        "SAFE AREA",
        egui::FontId::proportional(11.0 * scale),
        egui::Color32::from_rgb(104, 231, 224),
    );

    if !mock_ui {
        return;
    }
    let ink = egui::Color32::from_rgb(243, 246, 252);
    let muted = egui::Color32::from_rgb(195, 205, 220);
    let edge = egui::Stroke::new(1.7 * scale, ink);
    let is_story = platform.contains("Stories") || platform == "WhatsApp Status";
    if is_story {
        for i in 0..4 {
            painter.rect_filled(
                egui::Rect::from_min_max(
                    point(0.04 + i as f32 * 0.235, 0.085),
                    point(0.255 + i as f32 * 0.235, 0.089),
                ),
                2.0,
                egui::Color32::from_white_alpha(if i == 0 { 235 } else { 95 }),
            );
        }
        painter.circle_filled(
            point(0.09, 0.125),
            12.0 * scale,
            egui::Color32::from_rgb(125, 101, 195),
        );
        painter.text(
            point(0.15, 0.125),
            egui::Align2::LEFT_CENTER,
            "@your_account",
            egui::FontId::proportional(12.0 * scale),
            ink,
        );
        painter.rect_stroke(
            egui::Rect::from_min_max(point(0.055, 0.92), point(0.82, 0.975)),
            13.0 * scale,
            edge,
            egui::StrokeKind::Inside,
        );
        painter.text(
            point(0.09, 0.948),
            egui::Align2::LEFT_CENTER,
            "Send a message…",
            egui::FontId::proportional(12.0 * scale),
            ink,
        );
        painter.text(
            point(0.89, 0.948),
            egui::Align2::CENTER_CENTER,
            "♡",
            egui::FontId::proportional(23.0 * scale),
            ink,
        );
        return;
    }

    // Approximate occupied UI, not an official platform screenshot. All elements
    // are painted over the preview and never enter the renderer's export target.
    let (actions, navigation): ([&str; 4], [&str; 5]) = match platform {
        "TikTok" => (
            ["LIKE", "CHAT", "SAVE", "SHARE"],
            ["Home", "Friends", "+", "Inbox", "Profile"],
        ),
        "YouTube Shorts" => (
            ["LIKE", "CHAT", "REMIX", "SHARE"],
            ["Home", "Shorts", "+", "Subs", "You"],
        ),
        "Instagram Reels" | "Facebook Reels" => (
            ["LIKE", "CHAT", "SHARE", "SAVE"],
            ["Home", "Search", "+", "Reels", "Profile"],
        ),
        "Snapchat Spotlight" => (
            ["LIKE", "CHAT", "SHARE", "REMIX"],
            ["Map", "Chat", "+", "Stories", "Spotlight"],
        ),
        "Pinterest" => (
            ["SAVE", "CHAT", "SHARE", "MORE"],
            ["Home", "Explore", "+", "Inbox", "Profile"],
        ),
        "X video" => (
            ["LIKE", "CHAT", "REPOST", "SHARE"],
            ["Home", "Search", "+", "Alerts", "Profile"],
        ),
        "LinkedIn video" => (
            ["LIKE", "CHAT", "REPOST", "SHARE"],
            ["Home", "Network", "+", "Alerts", "Jobs"],
        ),
        "Threads / Bluesky video" => (
            ["LIKE", "CHAT", "REPOST", "SHARE"],
            ["Home", "Search", "+", "Alerts", "Profile"],
        ),
        "Twitch vertical clips" => (
            ["LIKE", "CHAT", "FOLLOW", "SHARE"],
            ["Follow", "Discover", "+", "Inbox", "Profile"],
        ),
        _ => (
            ["LIKE", "CHAT", "SAVE", "SHARE"],
            ["Home", "Explore", "+", "Inbox", "Profile"],
        ),
    };
    let rail_x = 0.91;
    painter.circle_filled(
        point(rail_x, 0.43),
        19.0 * scale,
        egui::Color32::from_rgb(93, 85, 153),
    );
    painter.circle_filled(
        point(rail_x + 0.033, 0.463),
        7.0 * scale,
        egui::Color32::from_rgb(235, 95, 161),
    );
    painter.text(
        point(rail_x + 0.033, 0.463),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(11.0 * scale),
        ink,
    );
    for (i, label) in actions.iter().enumerate() {
        let y = 0.54 + i as f32 * 0.085;
        let center = point(rail_x, y);
        match i {
            0 => {
                let a = center + egui::vec2(-7.0 * scale, -3.0 * scale);
                let b = center + egui::vec2(7.0 * scale, -3.0 * scale);
                painter.circle_stroke(a, 7.0 * scale, edge);
                painter.circle_stroke(b, 7.0 * scale, edge);
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        center + egui::vec2(-14.0 * scale, 0.0),
                        center + egui::vec2(14.0 * scale, 0.0),
                        center + egui::vec2(0.0, 15.0 * scale),
                    ],
                    ink,
                    egui::Stroke::NONE,
                ));
            }
            1 => {
                painter.rect_stroke(
                    egui::Rect::from_center_size(center, egui::vec2(29.0 * scale, 21.0 * scale)),
                    7.0 * scale,
                    edge,
                    egui::StrokeKind::Inside,
                );
                painter.line_segment(
                    [
                        center + egui::vec2(-4.0 * scale, 10.0 * scale),
                        center + egui::vec2(-9.0 * scale, 15.0 * scale),
                    ],
                    edge,
                );
            }
            2 => {
                painter.rect_stroke(
                    egui::Rect::from_center_size(center, egui::vec2(21.0 * scale, 28.0 * scale)),
                    2.0 * scale,
                    edge,
                    egui::StrokeKind::Inside,
                );
            }
            _ => {
                painter.line_segment(
                    [
                        center + egui::vec2(-12.0 * scale, 7.0 * scale),
                        center + egui::vec2(11.0 * scale, -7.0 * scale),
                    ],
                    edge,
                );
                painter.line_segment(
                    [
                        center + egui::vec2(4.0 * scale, -12.0 * scale),
                        center + egui::vec2(12.0 * scale, -7.0 * scale),
                    ],
                    edge,
                );
                painter.line_segment(
                    [
                        center + egui::vec2(12.0 * scale, -7.0 * scale),
                        center + egui::vec2(7.0 * scale, 2.0 * scale),
                    ],
                    edge,
                );
            }
        }
        painter.text(
            point(rail_x, y + 0.031),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(10.0 * scale),
            ink,
        );
    }
    painter.rect_filled(
        egui::Rect::from_min_max(point(0.0, 0.82), point(1.0, 1.0)),
        0.0,
        egui::Color32::from_black_alpha(150),
    );
    painter.text(
        point(0.06, 0.85),
        egui::Align2::LEFT_CENTER,
        "@your_account",
        egui::FontId::proportional(14.0 * scale),
        ink,
    );
    painter.text(
        point(0.06, 0.885),
        egui::Align2::LEFT_CENTER,
        "Your caption appears here",
        egui::FontId::proportional(12.0 * scale),
        ink,
    );
    painter.text(
        point(0.06, 0.913),
        egui::Align2::LEFT_CENTER,
        "#tags  ·  sound / music",
        egui::FontId::proportional(11.0 * scale),
        muted,
    );
    painter.line_segment(
        [point(0.0, 0.944), point(1.0, 0.944)],
        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(70)),
    );
    for (x, label) in [0.12, 0.32, 0.51, 0.70, 0.88].into_iter().zip(navigation) {
        painter.text(
            point(x, 0.967),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0 * scale),
            ink,
        );
    }
}
