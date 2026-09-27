use super::*;

const INK: egui::Color32 = egui::Color32::from_rgb(232, 239, 249);
const MUTED: egui::Color32 = egui::Color32::from_rgb(151, 165, 186);
const CYAN: egui::Color32 = egui::Color32::from_rgb(104, 231, 224);
const VIOLET: egui::Color32 = egui::Color32::from_rgb(170, 142, 255);

pub(super) fn configure_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = egui::Color32::from_rgb(15, 20, 32);
    style.visuals.window_fill = egui::Color32::from_rgb(20, 26, 40);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(10, 14, 24);
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(25, 32, 48);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(36, 46, 65);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(57, 70, 94);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(46, 101, 111);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(41, 109, 119);
    style.visuals.selection.stroke.color = INK;
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.interact_size.y = 32.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(21.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(14.0));
    ctx.set_style_of(egui::Theme::Dark, style);
}

impl Studio {
    pub(super) fn render_ui(&mut self, root_ui: &mut egui::Ui) {
        let ctx = root_ui.ctx().clone();
        // The preference is a multiplier of the operating system's display scale.
        // Replacing native pixels-per-point with 1.0 made Windows text tiny.
        let native_scale = ctx.native_pixels_per_point().unwrap_or(1.0);
        ctx.set_pixels_per_point(native_scale * self.ui_scale);

        if let Some(path) = self.startup_model.take() {
            if let Err(error) = self.load_model(&path) {
                self.notice = error.to_string();
            }
        }
        if let Some(path) = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path())
                .find(|path| {
                    path.to_string_lossy()
                        .to_ascii_lowercase()
                        .ends_with(".model3.json")
                })
                .map(Path::to_path_buf)
        }) {
            self.path_field = path.display().to_string();
            if let Err(error) = self.load_model(&path) {
                self.notice = error.to_string();
            }
        }
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame).as_secs_f64();
        let dt = elapsed.min(0.1);
        self.last_frame = now;
        self.process_network();
        self.poll_voice();
        if self
            .physics_dirty
            .is_some_and(|changed| changed.elapsed() >= Duration::from_millis(400))
        {
            if let Err(error) = self.save_physics_settings() {
                self.notice = error.to_string();
            }
        }
        self.update_model(dt, elapsed);
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

        egui::Panel::top("studio_header").show(root_ui, |ui| {
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("V").size(27.0).strong().color(VIOLET));
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("VALKYRIE STUDIO")
                            .size(20.0)
                            .strong()
                            .color(INK),
                    );
                    ui.label(
                        egui::RichText::new("Avatar direction · Voice · Capture")
                            .small()
                            .color(MUTED),
                    );
                });
                ui.add_space(22.0);
                let model_name = self
                    .model_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map_or("No model loaded".to_owned(), |n| {
                        n.to_string_lossy().to_string()
                    });
                ui.label(
                    egui::RichText::new(model_name).color(if self.model.is_some() {
                        CYAN
                    } else {
                        MUTED
                    }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Fullscreen  F11").clicked() {
                        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
                    }
                });
            });
            ui.add_space(7.0);
        });

        egui::Panel::bottom("studio_status").show(root_ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("●").color(if self.model.is_some() {
                    CYAN
                } else {
                    MUTED
                }));
                ui.label(egui::RichText::new(&self.notice).color(INK));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{:.0} FPS  ·  model {:.1} ms  ·  render {:.1} ms",
                            self.fps, self.model_ms, self.render_ms
                        ))
                        .small()
                        .color(MUTED),
                    );
                });
            });
        });

        egui::Panel::left("workspace_controls")
            .default_size(372.0)
            .min_size(320.0)
            .max_size(520.0)
            .resizable(true)
            .show(root_ui, |ui| {
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    for (tab, label) in [
                        (StudioTab::Stage, "Stage"),
                        (StudioTab::Physics, "Physics"),
                        (StudioTab::Voice, "Voice"),
                        (StudioTab::Capture, "Capture"),
                        (StudioTab::Inputs, "Inputs"),
                    ] {
                        if ui
                            .selectable_label(self.selected_tab == tab, label)
                            .clicked()
                        {
                            self.selected_tab = tab;
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match self.selected_tab {
                            StudioTab::Stage => self.stage_controls(ui),
                            StudioTab::Physics => self.physics_controls(ui),
                            StudioTab::Voice => self.voice_controls(ui),
                            StudioTab::Capture => self.capture_controls(ui),
                            StudioTab::Inputs => self.input_controls(ui),
                        }
                    });
            });

        egui::CentralPanel::default().show(root_ui, |ui| self.stage_preview(ui));
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn section(ui: &mut egui::Ui, title: &str, detail: &str) {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(title).size(18.0).strong().color(INK));
        ui.label(egui::RichText::new(detail).small().color(MUTED));
        ui.add_space(5.0);
    }

    fn stage_controls(&mut self, ui: &mut egui::Ui) {
        Self::section(
            ui,
            "Model",
            "Open or drop a .model3.json; keep its textures beside it.",
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.path_field)
                .desired_width(f32::INFINITY)
                .hint_text("Path to model3.json"),
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button("Browse models…").clicked() {
                let mut dialog = rfd::FileDialog::new().add_filter("Avatar model", &["json"]);
                if let Some(parent) = self.model_path.as_ref().and_then(|p| p.parent()) {
                    dialog = dialog.set_directory(parent);
                }
                if let Some(path) = dialog.pick_file() {
                    self.path_field = path.display().to_string();
                    if let Err(error) = self.load_model(&path) {
                        self.notice = error.to_string();
                    }
                }
            }
            if ui.button("Load path").clicked() {
                let path = PathBuf::from(self.path_field.trim());
                if let Err(error) = self.load_model(&path) {
                    self.notice = error.to_string();
                }
            }
        });
        if !self.recent_models.is_empty() {
            ui.collapsing("Recent models", |ui| {
                for path in self.recent_models.clone() {
                    let name = path.file_stem().unwrap_or_default().to_string_lossy();
                    if ui
                        .button(name.as_ref())
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        self.path_field = path.display().to_string();
                        if let Err(error) = self.load_model(&path) {
                            self.notice = error.to_string();
                        }
                    }
                }
            });
        }
        ui.separator();
        Self::section(ui, "Framing", "Set the shot before you record.");
        ui.horizontal_wrapped(|ui| {
            for (label, size) in [
                ("Vertical 9:16", [1080, 1920]),
                ("Horizontal 16:9", [1920, 1080]),
            ] {
                if ui
                    .add_enabled(
                        !self.is_recording(),
                        egui::Button::new(label).selected(self.canvas == size),
                    )
                    .clicked()
                {
                    if let Err(error) = self.change_canvas(size[0], size[1]) {
                        self.notice = error.to_string();
                    }
                }
            }
        });
        ui.add(egui::Slider::new(&mut self.zoom, 0.1..=30.0).text("Zoom"));
        ui.add(egui::Slider::new(&mut self.pan.x, -12.0..=12.0).text("Pan X"));
        ui.add(egui::Slider::new(&mut self.pan.y, -12.0..=12.0).text("Pan Y"));
        if ui.button("Reset framing").clicked() {
            self.zoom = 1.0;
            self.pan = egui::Vec2::ZERO;
        }
        ui.separator();
        Self::section(
            ui,
            "Social preview",
            "Preview-only platform UI and safe areas.",
        );
        egui::ComboBox::from_id_salt("guide")
            .selected_text(self.guide)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for name in std::iter::once("All platforms")
                    .chain(guides::PRESETS.iter().map(|(name, _)| *name))
                    .chain(std::iter::once("Custom"))
                {
                    ui.selectable_value(&mut self.guide, name, name);
                }
            });
        ui.checkbox(&mut self.show_guides, "Show safe areas");
        ui.add_enabled_ui(self.show_guides, |ui| {
            ui.checkbox(
                &mut self.mock_ui,
                "Show platform controls, chat and captions",
            );
            ui.add(egui::Slider::new(&mut self.guide_opacity, 0.0..=0.8).text("Shade"));
            if self.guide == "Custom" {
                for (i, label) in ["Left", "Top", "Right", "Bottom"].iter().enumerate() {
                    ui.add(egui::Slider::new(&mut self.guide_margins[i], 0.0..=0.45).text(*label));
                }
            }
        });
        ui.separator();
        Self::section(
            ui,
            "Appearance",
            "Follows desktop display scaling by default.",
        );
        ui.add(egui::Slider::new(&mut self.ui_scale, 0.75..=2.5).text("UI size"));
        if ui.button("Use desktop size").clicked() {
            self.ui_scale = 1.0;
        }
    }

    fn physics_controls(&mut self, ui: &mut egui::Ui) {
        let Some(physics) = &self.physics else {
            Self::section(
                ui,
                "Physics",
                "Load a model with a .physics3.json file to tune its particle groups.",
            );
            return;
        };
        let groups = physics.groups();
        let before = self.physics_settings.clone();
        Self::section(
            ui,
            "Physics",
            "Tune motion without changing the model's files. Settings are saved for this model.",
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button("Settle motion").clicked() {
                if let Some(physics) = &mut self.physics {
                    physics.reset();
                }
            }
            if ui.button("Save settings").clicked() {
                match self.save_physics_settings() {
                    Ok(()) => self.notice = "Physics settings saved.".into(),
                    Err(error) => self.notice = error.to_string(),
                }
            }
        });
        ui.separator();
        ui.checkbox(&mut self.physics_settings.enabled, "Enable avatar physics");
        ui.add_enabled_ui(self.physics_settings.enabled, |ui| {
            egui::ComboBox::from_id_salt("physics-motion-style")
                .selected_text(match self.physics_settings.motion_style {
                    MotionStyle::Bouncy => "Bouncy · expressive rebound",
                    MotionStyle::Natural => "Natural · faster settling",
                    MotionStyle::Authored => "Authored · legacy A.R.I.A",
                })
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.physics_settings.motion_style,
                        MotionStyle::Bouncy,
                        "Bouncy · expressive rebound",
                    );
                    ui.selectable_value(
                        &mut self.physics_settings.motion_style,
                        MotionStyle::Natural,
                        "Natural · faster settling",
                    );
                    ui.selectable_value(
                        &mut self.physics_settings.motion_style,
                        MotionStyle::Authored,
                        "Authored · legacy A.R.I.A",
                    );
                });
            ui.small(match self.physics_settings.motion_style {
                MotionStyle::Bouncy => "Longer swings. Lower tail inertia to calm oscillation.",
                MotionStyle::Natural => "Smoother motion that comes to rest sooner.",
                MotionStyle::Authored => {
                    "Uses the model's authored frame rate and A.R.I.A's earlier solver."
                }
            });
        });
        physics_sliders(
            ui,
            self.physics_settings.enabled,
            &mut self.physics_settings.strength,
            &mut self.physics_settings.inertia,
            &mut self.physics_settings.response,
            &mut self.physics_settings.gravity,
            &mut self.physics_settings.wind,
        );
        if ui.button("Reset overall").clicked() {
            let groups = std::mem::take(&mut self.physics_settings.groups);
            self.physics_settings = self.physics_defaults.clone();
            self.physics_settings.groups = groups;
            if let Some(physics) = &mut self.physics {
                physics.reset();
            }
        }
        ui.separator();
        Self::section(
            ui,
            &format!("Model groups · {}", groups.len()),
            "Search and tune individual tail, hair, ear, clothing and accessory chains.",
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.physics_search)
                .hint_text("Find a group or parameter…")
                .desired_width(f32::INFINITY),
        );
        ui.checkbox(&mut self.physics_modified_only, "Only modified groups");
        ui.collapsing("Group actions", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Enable all").clicked() {
                    for info in &groups {
                        self.physics_settings
                            .groups
                            .entry(info.id.clone())
                            .or_default()
                            .enabled = true;
                    }
                }
                if ui.button("Disable all").clicked() {
                    for info in &groups {
                        self.physics_settings
                            .groups
                            .entry(info.id.clone())
                            .or_default()
                            .enabled = false;
                    }
                }
                if ui.button("Reset all groups").clicked() {
                    self.physics_settings.groups = self.physics_defaults.groups.clone();
                    if let Some(physics) = &mut self.physics {
                        physics.reset();
                    }
                }
            });
        });
        let search = self.physics_search.trim().to_ascii_lowercase();
        let mut shown = 0;
        for info in &groups {
            let baseline = self
                .physics_defaults
                .groups
                .get(&info.id)
                .copied()
                .unwrap_or_default();
            let mut tuning = self
                .physics_settings
                .groups
                .get(&info.id)
                .copied()
                .unwrap_or_default();
            if self.physics_modified_only && tuning == baseline {
                continue;
            }
            let haystack = format!(
                "{} {} {} {}",
                info.name,
                info.id,
                info.inputs.join(" "),
                info.outputs.join(" ")
            )
            .to_ascii_lowercase();
            if !haystack.contains(&search) {
                continue;
            }
            shown += 1;
            ui.push_id(&info.id, |ui| {
                ui.collapsing(&info.name, |ui| {
                    ui.small(format!(
                        "{} inputs · {} outputs · {} particles",
                        info.inputs.len(),
                        info.outputs.len(),
                        info.particles
                    ));
                    ui.checkbox(&mut tuning.enabled, "Enable group");
                    physics_sliders(
                        ui,
                        tuning.enabled && self.physics_settings.enabled,
                        &mut tuning.strength,
                        &mut tuning.inertia,
                        &mut tuning.response,
                        &mut tuning.gravity,
                        &mut tuning.wind,
                    );
                    if ui.button("Reset this group").clicked() {
                        tuning = baseline;
                    }
                    ui.collapsing("Authored connections", |ui| {
                        ui.small(format!(
                            "ID: {} · imported multiplier {:.2}×",
                            info.id, info.imported_multiplier
                        ));
                        ui.label("Driven by");
                        for input in &info.inputs {
                            ui.small(input);
                        }
                        ui.label("Moves");
                        for output in &info.outputs {
                            ui.small(output);
                        }
                    });
                });
            });
            if tuning == GroupSettings::default() && baseline == GroupSettings::default() {
                self.physics_settings.groups.remove(&info.id);
            } else {
                self.physics_settings.groups.insert(info.id.clone(), tuning);
            }
        }
        if shown == 0 {
            ui.small("No groups match this filter.");
        }
        if self.physics_settings != before {
            if let Some(physics) = &mut self.physics {
                physics.configure(&self.physics_settings);
            }
            self.physics_dirty = Some(Instant::now());
        }
    }

    fn voice_controls(&mut self, ui: &mut egui::Ui) {
        Self::section(
            ui,
            "Voice",
            "Type a line, pick a voice, then animate speech.",
        );
        ui.horizontal(|ui| {
            for provider in ["elevenlabs", "openai"] {
                let label = if provider == "elevenlabs" {
                    "ElevenLabs"
                } else {
                    "OpenAI"
                };
                if ui
                    .selectable_label(self.voice_provider == provider, label)
                    .clicked()
                {
                    self.voice_provider = provider;
                }
            }
        });
        ui.add_space(5.0);
        if self.voice_provider == "elevenlabs" {
            ui.label(if self.voice.elevenlabs_key.is_some() {
                "ElevenLabs key ready"
            } else {
                "Add your ElevenLabs key"
            });
            if ui.button("Refresh my voices").clicked() {
                if let Err(error) = self.refresh_voices() {
                    self.notice = error.to_string();
                }
            }
            let current = self
                .voices
                .iter()
                .find(|v| v.voice_id == self.voice.elevenlabs_voice)
                .map_or("Select a voice", |v| v.name.as_str());
            egui::ComboBox::from_id_salt("eleven-voices")
                .selected_text(current)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for choice in &self.voices {
                        ui.selectable_value(
                            &mut self.voice.elevenlabs_voice,
                            choice.voice_id.clone(),
                            &choice.name,
                        );
                    }
                });
            ui.label(egui::RichText::new("ElevenLabs model").small().color(MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut self.voice.elevenlabs_model)
                    .desired_width(f32::INFINITY),
            );
        } else {
            ui.label(if self.voice.openai_key.is_some() {
                "OpenAI key ready"
            } else {
                "Add your OpenAI key"
            });
            egui::ComboBox::from_id_salt("openai-voices")
                .selected_text(&self.voice.openai_voice)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for choice in voice::OPENAI_VOICES {
                        ui.selectable_value(
                            &mut self.voice.openai_voice,
                            (*choice).into(),
                            *choice,
                        );
                    }
                });
            ui.add(egui::Slider::new(&mut self.voice.speed, 0.25..=4.0).text("Speed"));
        }
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.key_field)
                .password(true)
                .desired_width(f32::INFINITY)
                .hint_text("Provider API key"),
        );
        if cfg!(windows) {
            ui.checkbox(&mut self.remember_key, "Remember on this Windows account");
        }
        ui.horizontal(|ui| {
            if ui.button("Use key").clicked() {
                match self.voice.set_key(
                    self.voice_provider,
                    &self.key_field,
                    &self.data_dir,
                    self.remember_key,
                ) {
                    Ok(()) => {
                        self.key_field.clear();
                        self.notice = "Voice key ready.".into();
                    }
                    Err(error) => self.notice = error.to_string(),
                }
            }
            if ui.button("Forget key").clicked() {
                if let Err(error) = self.voice.forget_key(self.voice_provider, &self.data_dir) {
                    self.notice = error.to_string();
                }
            }
        });
        ui.separator();
        Self::section(
            ui,
            "Dialogue",
            "Speech drives the model's mouth while audio plays.",
        );
        ui.label(egui::RichText::new("How it works: type a line, choose ElevenLabs or OpenAI and a voice above, then press Speak and animate. The service generates audio; Valkyrie plays it and uses its timing and volume to move the model's mouth. Select Agent in Inputs to add head and body motion at the same time. Recording captures the animated model and audio; an API key is needed for the chosen voice service.").small().color(MUTED));
        ui.add(
            egui::TextEdit::multiline(&mut self.speech_text)
                .desired_rows(7)
                .desired_width(f32::INFINITY)
                .hint_text("Write what Vaelari should say…"),
        );
        if ui
            .add_enabled(
                !self.voice_busy,
                egui::Button::new(if self.voice_busy {
                    "Generating…"
                } else {
                    "Speak and animate"
                }),
            )
            .clicked()
        {
            let provider = self.voice_provider;
            let text = self.speech_text.clone();
            let config = self.voice.clone();
            if let Err(error) = self.start_tts(provider, &text, config, true) {
                self.notice = error.to_string();
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Replay").clicked() {
                if let Err(error) = self.play_audio() {
                    self.notice = error.to_string();
                }
            }
            if ui.button("Stop audio").clicked() {
                if let Some(player) = &mut self.audio_player {
                    player.stop();
                }
            }
        });
    }

    fn capture_controls(&mut self, ui: &mut egui::Ui) {
        Self::section(
            ui,
            "Capture",
            "Export the model canvas without preview guides.",
        );
        ui.label(
            egui::RichText::new(format!("Canvas  {} × {}", self.canvas[0], self.canvas[1]))
                .color(CYAN),
        );
        ui.horizontal_wrapped(|ui| {
            for (label, size) in [
                ("Vertical 9:16", [1080, 1920]),
                ("Horizontal 16:9", [1920, 1080]),
            ] {
                if ui
                    .add_enabled(
                        !self.is_recording(),
                        egui::Button::new(label).selected(self.canvas == size),
                    )
                    .clicked()
                {
                    if let Err(error) = self.change_canvas(size[0], size[1]) {
                        self.notice = error.to_string();
                    }
                }
            }
        });
        ui.add_space(7.0);
        ui.label(egui::RichText::new("Output file").small().color(MUTED));
        ui.add(
            egui::TextEdit::singleline(&mut self.record_output)
                .desired_width(f32::INFINITY)
                .hint_text("C:\\Videos\\vaelari.mp4"),
        );
        egui::ComboBox::from_id_salt("record-codec")
            .selected_text(self.record_codec)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for codec in ["h264", "h265", "prores", "vp9"] {
                    ui.selectable_value(&mut self.record_codec, codec, codec);
                }
            });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.is_recording(), egui::Button::new("Start recording"))
                .clicked()
            {
                let path = PathBuf::from(self.record_output.trim());
                if let Err(error) = self.start_recording(path, 30, self.record_codec) {
                    self.notice = error.to_string();
                }
            }
            if ui
                .add_enabled(self.is_recording(), egui::Button::new("Stop recording"))
                .clicked()
            {
                if let Some(export) = &mut self.export {
                    export.stop();
                }
            }
        });
        if let Some(export) = &self.export {
            let stats = export.stats();
            ui.label(format!(
                "{} · {} frames · {} dropped",
                stats.state, stats.frames, stats.dropped
            ));
        }
        ui.separator();
        ui.label(egui::RichText::new("Guides and platform mock controls are preview-only. They are not baked into the recording.").small().color(MUTED));
    }

    fn input_controls(&mut self, ui: &mut egui::Ui) {
        Self::section(ui, "Live control", "Choose who drives the model.");
        ui.horizontal_wrapped(|ui| {
            for mode in ["agent", "phone", "webcam", "idle"] {
                if ui
                    .selectable_label(self.engine.mode == mode, mode)
                    .clicked()
                {
                    let _ = self.engine.set_mode(mode);
                }
            }
        });
        if let Some(active) = &self.network {
            ui.label(egui::RichText::new(format!("Agent API  {}", active.api_address)).color(CYAN));
        }
        ui.separator();
        Self::section(
            ui,
            "Phone and webcam",
            "Connect a UDP tracker to this computer.",
        );
        ui.label(egui::RichText::new("Listen address").small().color(MUTED));
        ui.add(
            egui::TextEdit::singleline(&mut self.tracking_bind_field).desired_width(f32::INFINITY),
        );
        ui.label(
            egui::RichText::new("UDP ports, comma separated")
                .small()
                .color(MUTED),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.tracking_ports_field).desired_width(f32::INFINITY),
        );
        if ui.button("Apply listening addresses").clicked() {
            if let Err(error) = self.restart_tracking() {
                self.notice = error.to_string();
            }
        }
        if let Some(active) = &self.network {
            ui.small(format!(
                "Listening: {}",
                active
                    .tracking_addresses
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("For a phone on your LAN, enter this PC's LAN IP as its destination and one listed port. The webcam helper sends to localhost:15483.").small().color(MUTED));
    }

    fn stage_preview(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("STAGE PREVIEW")
                    .size(15.0)
                    .strong()
                    .color(INK),
            );
            ui.label(
                egui::RichText::new(format!("{} × {}", self.canvas[0], self.canvas[1]))
                    .small()
                    .color(MUTED),
            );
            if self.show_guides {
                ui.label(
                    egui::RichText::new(format!("· {} guide", self.guide))
                        .small()
                        .color(CYAN),
                );
            }
        });
        ui.separator();
        let bounds = ui.available_rect_before_wrap();
        let painter = ui.painter();
        painter.rect_filled(bounds, 12.0, egui::Color32::from_rgb(10, 15, 26));
        let content = bounds.shrink2(egui::vec2(18.0, 18.0));
        let aspect = self.canvas[0] as f32 / self.canvas[1] as f32;
        let width = (content.height() * aspect).min(content.width());
        let portrait =
            egui::Rect::from_center_size(content.center(), egui::vec2(width, width / aspect));
        painter.rect_filled(
            portrait.expand(4.0),
            7.0,
            egui::Color32::from_rgb(57, 70, 94),
        );
        painter.rect_filled(portrait, 3.0, egui::Color32::from_rgb(28, 30, 43));
        if let Some(renderer) = &self.renderer {
            let rect = renderer.image.rect(portrait, 1.0);
            painter.image(
                renderer.image.id,
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            painter.text(
                portrait.center() - egui::vec2(0.0, 15.0),
                egui::Align2::CENTER_CENTER,
                "Your model appears here",
                egui::FontId::proportional(21.0),
                INK,
            );
            painter.text(
                portrait.center() + egui::vec2(0.0, 17.0),
                egui::Align2::CENTER_CENTER,
                "Choose a .model3.json in Stage",
                egui::FontId::proportional(14.0),
                MUTED,
            );
        }
        if self.show_guides && aspect < 0.8 {
            super::draw_guide(
                painter,
                portrait,
                self.guide,
                self.guide_margins,
                self.mock_ui,
                self.guide_opacity,
            );
        }
    }
}

fn physics_sliders(
    ui: &mut egui::Ui,
    enabled: bool,
    strength: &mut f32,
    inertia: &mut f32,
    response: &mut f32,
    gravity: &mut f32,
    wind: &mut f32,
) {
    ui.add_enabled_ui(enabled, |ui| {
        ui.add(egui::Slider::new(strength, 0.0..=2.0).text("Strength"));
        ui.add(egui::Slider::new(inertia, 0.0..=2.0).text("Inertia"));
        ui.add(egui::Slider::new(response, 0.25..=2.0).text("Response speed"));
        ui.add(egui::Slider::new(gravity, 0.0..=2.0).text("Gravity"));
        ui.add(egui::Slider::new(wind, -1.0..=1.0).text("Wind"));
    });
}
