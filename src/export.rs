//! Bounded GPU readback and lossless-first FFmpeg export. The render thread
//! never waits for video compression; slow capture ticks repeat the prior frame.
use crate::renderer::{ModelRenderer, PendingReadback};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender, TrySendError},
    },
    thread,
    time::Instant,
};

#[derive(Debug, Clone, Serialize)]
pub struct ExportStats {
    pub state: String,
    pub frames: u64,
    pub captured: u64,
    pub dropped: u64,
    pub duplicates: u64,
    pub output: PathBuf,
    pub error: Option<String>,
}
enum Message {
    Frame(u64, Vec<u8>),
    Stop(u64),
}

pub struct Export {
    sender: SyncSender<Message>,
    pending: VecDeque<(u64, PendingReadback)>,
    stats: Arc<Mutex<ExportStats>>,
    started: Instant,
    fps: u32,
    last_index: Option<u64>,
    stop_requested: bool,
    stop_sent: bool,
    worker: Option<thread::JoinHandle<()>>,
}

fn codec_options(codec: &str, output: &Path) -> Result<Vec<&'static str>> {
    let extension = output
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match (codec, extension.as_str()) {
        ("h264", "mp4") => Ok(vec![
            "-c:v",
            "libx264",
            "-crf",
            "16",
            "-preset",
            "medium",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ]),
        ("h265", "mp4") => Ok(vec![
            "-c:v", "libx265", "-crf", "18", "-pix_fmt", "yuv420p", "-tag:v", "hvc1",
        ]),
        ("prores", "mov") => Ok(vec![
            "-c:v",
            "prores_ks",
            "-profile:v",
            "4",
            "-pix_fmt",
            "yuva444p10le",
            "-alpha_bits",
            "16",
        ]),
        ("vp9", "webm") => Ok(vec![
            "-c:v",
            "libvpx-vp9",
            "-pix_fmt",
            "yuva420p",
            "-b:v",
            "0",
            "-crf",
            "18",
            "-auto-alt-ref",
            "0",
            "-metadata:s:v:0",
            "alpha_mode=1",
            "-deadline",
            "good",
            "-cpu-used",
            "4",
            "-row-mt",
            "1",
        ]),
        _ => bail!("Choose h264/h265 .mp4, prores .mov, or vp9 .webm"),
    }
}
fn partial_path(output: &Path) -> PathBuf {
    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let extension = output.extension().unwrap_or_default().to_string_lossy();
    output.with_file_name(format!("{stem}.partial.{extension}"))
}

fn required_encoder(codec: &str) -> &'static str {
    match codec {
        "h264" => "libx264",
        "h265" => "libx265",
        "prores" => "prores_ks",
        "vp9" => "libvpx-vp9",
        _ => unreachable!(),
    }
}

impl Export {
    pub fn start(
        renderer: &ModelRenderer,
        output: PathBuf,
        fps: u32,
        codec: &str,
        audio: Option<PathBuf>,
    ) -> Result<Self> {
        ensure!((1..=120).contains(&fps), "Export FPS must be 1–120");
        let _ = codec_options(codec, &output)?;
        ensure!(
            !output.exists() && !partial_path(&output).exists(),
            "Output or partial file already exists"
        );
        ensure!(
            (renderer.image.size.x as u32).is_multiple_of(2)
                && (renderer.image.size.y as u32).is_multiple_of(2),
            "Canvas dimensions must be even"
        );
        let ffmpeg = std::env::var_os("FFMPEG").unwrap_or_else(|| "ffmpeg".into());
        let probe = Command::new(&ffmpeg)
            .args(["-hide_banner", "-encoders"])
            .output()
            .context(
                "FFmpeg is required for recording; install it or set FFMPEG to its full path",
            )?;
        ensure!(
            probe.status.success(),
            "FFmpeg could not list available encoders"
        );
        let encoders = String::from_utf8_lossy(&probe.stdout);
        let required = required_encoder(codec);
        ensure!(
            encoders.contains(" utvideo ") && encoders.contains(&format!(" {required} ")),
            "This FFmpeg build needs both utvideo and {required} encoders for {codec} export"
        );
        fs::create_dir_all(output.parent().context("Output has no parent")?)?;
        // 16 frames absorb short encoder stalls without letting a long session
        // grow unbounded (about 127 MiB at 1080x1920 RGBA).
        let (sender, receiver) = mpsc::sync_channel(16);
        let stats = Arc::new(Mutex::new(ExportStats {
            state: "recording".into(),
            frames: 0,
            captured: 0,
            dropped: 0,
            duplicates: 0,
            output: output.clone(),
            error: None,
        }));
        let thread_stats = Arc::clone(&stats);
        let codec = codec.to_owned();
        let width = renderer.image.size.x as u32;
        let height = renderer.image.size.y as u32;
        let worker = thread::spawn(move || {
            if let Err(error) = encode(
                &ffmpeg,
                &output,
                width,
                height,
                fps,
                &codec,
                audio.as_deref(),
                receiver,
                &thread_stats,
            ) {
                if let Ok(mut stats) = thread_stats.lock() {
                    stats.state = "failed".into();
                    stats.error = Some(error.to_string());
                }
            }
        });
        Ok(Self {
            sender,
            pending: VecDeque::new(),
            stats,
            started: Instant::now(),
            fps,
            last_index: None,
            stop_requested: false,
            stop_sent: false,
            worker: Some(worker),
        })
    }
    pub fn stop(&mut self) {
        self.stop_requested = true;
        if let Ok(mut stats) = self.stats.lock() {
            if stats.state == "recording" {
                stats.state = "draining".into();
            }
        }
    }
    pub fn stats(&self) -> ExportStats {
        self.stats
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| ExportStats {
                state: "failed".into(),
                frames: 0,
                captured: 0,
                dropped: 0,
                duplicates: 0,
                output: PathBuf::new(),
                error: Some("Export state unavailable".into()),
            })
    }
    pub fn pump(&mut self, renderer: &ModelRenderer) -> Result<()> {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            return Ok(());
        }
        if !self.stop_requested && !self.stop_sent {
            let index = (self.started.elapsed().as_secs_f64() * self.fps as f64).floor() as u64;
            if self.last_index.is_none_or(|last| index > last) {
                self.last_index = Some(index);
                if self.pending.len() < 8 {
                    self.pending.push_back((index, renderer.begin_capture()?));
                } else if let Ok(mut stats) = self.stats.lock() {
                    stats.dropped += 1;
                }
            }
        }
        while let Some((index, pending)) = self.pending.front() {
            let Some(rgba) = pending.try_finish()? else {
                break;
            };
            let index = *index;
            self.pending.pop_front();
            match self.sender.try_send(Message::Frame(index, rgba)) {
                Ok(()) => {
                    if let Ok(mut stats) = self.stats.lock() {
                        stats.captured += 1;
                    }
                }
                Err(TrySendError::Full(_)) => {
                    if let Ok(mut stats) = self.stats.lock() {
                        stats.dropped += 1;
                    }
                }
                Err(TrySendError::Disconnected(_)) => bail!("Encoder stopped unexpectedly"),
            }
        }
        if self.stop_requested && !self.stop_sent && self.pending.is_empty() {
            let target = self.last_index.map_or(0, |v| v + 1);
            match self.sender.try_send(Message::Stop(target)) {
                Ok(()) => self.stop_sent = true,
                Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => bail!("Encoder stopped unexpectedly"),
            }
        }
        Ok(())
    }
    pub fn finished(&mut self) -> bool {
        if self.worker.as_ref().is_some_and(|h| h.is_finished()) {
            let _ = self.worker.take().unwrap().join();
            true
        } else {
            false
        }
    }
}

fn unpremultiply(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] == 0 {
            pixel[..3].fill(0);
        } else if pixel[3] != 255 {
            pixel.copy_from_slice(&crate::renderer::straight_alpha([
                pixel[0], pixel[1], pixel[2], pixel[3],
            ]));
        }
    }
}

fn encode(
    ffmpeg: &std::ffi::OsStr,
    output: &Path,
    width: u32,
    height: u32,
    fps: u32,
    codec: &str,
    audio: Option<&Path>,
    receiver: mpsc::Receiver<Message>,
    stats: &Arc<Mutex<ExportStats>>,
) -> Result<()> {
    let intermediate = output.with_file_name(format!(
        "{}.capture.mkv",
        output.file_name().unwrap_or_default().to_string_lossy()
    ));
    ensure!(
        !intermediate.exists(),
        "Capture intermediate already exists"
    );
    let trace = std::env::var_os("VALKYRIE_EXPORT_TRACE").map(PathBuf::from);
    let mut child = Command::new(ffmpeg)
        .args([
            "-v",
            if trace.is_some() { "debug" } else { "warning" },
            "-nostdin",
            "-n",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
        ])
        .arg(format!("{width}x{height}"))
        .args(["-framerate"])
        .arg(fps.to_string())
        .args([
            "-i", "pipe:0", "-map", "0:v:0", "-c:v", "utvideo", "-pix_fmt", "gbrap", "-pred",
            "left", "-threads", "4",
        ])
        .arg(&intermediate)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().context("FFmpeg stdin unavailable")?;
    let stderr = child.stderr.take().context("FFmpeg stderr unavailable")?;
    let errors = thread::spawn(move || {
        let mut stderr = stderr;
        let mut trace_file = trace.and_then(|path| fs::File::create(path).ok());
        let mut tail = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(read) = stderr.read(&mut chunk) {
            if read == 0 {
                break;
            }
            if let Some(file) = &mut trace_file {
                let _ = file.write_all(&chunk[..read]);
            }
            tail.extend_from_slice(&chunk[..read]);
            if tail.len() > 16384 {
                tail.drain(..tail.len() - 16384);
            }
        }
        String::from_utf8_lossy(&tail).to_string()
    });
    let mut frames = 0u64;
    let mut previous: Option<Vec<u8>> = None;
    let mut target = None;
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Frame(index, mut frame) => {
                ensure!(
                    frame.len() == width as usize * height as usize * 4,
                    "Invalid GPU frame length"
                );
                unpremultiply(&mut frame);
                while frames < index {
                    stdin.write_all(previous.as_deref().unwrap_or(&frame))?;
                    frames += 1;
                    if let Ok(mut s) = stats.lock() {
                        s.duplicates += 1;
                    }
                }
                stdin.write_all(&frame)?;
                frames += 1;
                previous = Some(frame);
                if let Ok(mut s) = stats.lock() {
                    s.frames = frames;
                }
            }
            Message::Stop(count) => {
                target = Some(count);
                break;
            }
        }
    }
    if let Some(count) = target {
        while frames < count {
            let last = previous
                .as_ref()
                .context("Recording stopped without a frame")?;
            stdin.write_all(last)?;
            frames += 1;
            if let Ok(mut s) = stats.lock() {
                s.duplicates += 1;
                s.frames = frames;
            }
        }
    }
    drop(stdin);
    let exit = child.wait()?;
    let error_log = errors.join().unwrap_or_default();
    ensure!(
        exit.success() && frames > 0,
        "FFmpeg capture failed: {}",
        error_log.chars().take(2000).collect::<String>()
    );
    if let Ok(mut s) = stats.lock() {
        s.state = "saving".into();
    }
    let partial = partial_path(output);
    let duration = format!("{:.9}", frames as f64 / fps as f64);
    let mut command = Command::new(ffmpeg);
    command
        .args(["-v", "error", "-nostdin", "-n", "-i"])
        .arg(&intermediate);
    if let Some(audio) = audio {
        command.arg("-i").arg(audio);
    }
    command
        .args(["-map", "0:v:0"])
        .args(codec_options(codec, output)?);
    if audio.is_some() {
        command
            .args([
                "-map",
                "1:a:0",
                "-c:a",
                if codec == "vp9" {
                    "libopus"
                } else if codec == "prores" {
                    "pcm_s16le"
                } else {
                    "aac"
                },
                "-af",
            ])
            .arg(format!("apad,atrim=end={duration},asetpts=PTS-STARTPTS"));
    }
    let result = command.arg("-t").arg(&duration).arg(&partial).output()?;
    ensure!(
        result.status.success(),
        "Final export failed: {}",
        String::from_utf8_lossy(&result.stderr)
            .chars()
            .take(2000)
            .collect::<String>()
    );
    fs::rename(&partial, output)?;
    let _ = fs::remove_file(intermediate);
    if let Ok(mut s) = stats.lock() {
        s.state = "saved".into();
        s.frames = frames;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codec_and_filename_match() {
        assert!(codec_options("h264", Path::new("video.mp4")).is_ok());
        assert!(codec_options("vp9", Path::new("video.mp4")).is_err());
        assert_eq!(
            partial_path(Path::new("video.mp4")),
            Path::new("video.partial.mp4")
        );
    }
    #[test]
    fn alpha_conversion_runs_after_gpu_readback() {
        let mut pixels = [64, 32, 0, 128, 1, 2, 3, 0, 255, 200, 100, 255];
        unpremultiply(&mut pixels);
        assert_eq!(pixels, [128, 64, 0, 128, 0, 0, 0, 0, 255, 200, 100, 255]);
    }
}
