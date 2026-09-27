use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    env,
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use valkyrie_studio::{
    app_paths,
    audio::AudioClip,
    performance::{Line, Script, validate_output},
};

const RATE: u32 = 24_000;

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let mut script_path = None;
    let mut output = None;
    let mut validate = false;
    while let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "--script" => script_path = args.next().map(PathBuf::from),
            "--output" => output = args.next().map(PathBuf::from),
            "--validate" => validate = true,
            "--help" | "-h" => {
                println!(
                    "Usage: valkyrie-perform --script performance.json --output new-video.mp4 [--validate]"
                );
                return Ok(());
            }
            _ => bail!("Unknown argument: {}", arg.to_string_lossy()),
        }
    }
    let script_path = script_path.context("Provide --script PATH")?;
    let script = Script::parse(&fs::read_to_string(&script_path)?)?;
    if validate {
        println!(
            "Valid: {} lines, {} voice, {}x{} at {} FPS",
            script.lines.len(),
            script.provider,
            script.width,
            script.height,
            script.fps
        );
        return Ok(());
    }
    let output = output.context("Provide --output PATH")?;
    let codec = validate_output(&output)?;
    ensure!(
        !output.exists(),
        "Output already exists; choose a new filename"
    );
    let output = absolute(&output)?;
    let work = output.with_file_name(format!(
        "{}.performance",
        output.file_name().unwrap_or_default().to_string_lossy()
    ));
    ensure!(!work.exists(), "Performance work folder already exists");
    fs::create_dir_all(&work)?;
    let mut session = Session::connect()?;
    if let Some(path) = &script.model_path {
        session.send(json!({"op":"load_model","path":path}))?;
    }
    session.send(json!({"op":"mode","mode":"agent"}))?;
    let status = session.send(json!({"op":"status"}))?;
    ensure!(
        status["agent"]["ready"] == true,
        "Load a model in Valkyrie Studio before rendering"
    );
    session.send(json!({"op":"canvas","width":script.width,"height":script.height}))?;
    session
        .send(json!({"op":"view","zoom":script.view.zoom,"x":script.view.x,"y":script.view.y}))?;
    let base = script_path.parent().unwrap_or(Path::new("."));
    let mut prepared = Vec::with_capacity(script.lines.len());
    for (index, line) in script.lines.iter().enumerate() {
        println!("Preparing voice line {}/{}", index + 1, script.lines.len());
        let source = if let Some(path) = &line.audio {
            if path.is_absolute() {
                path.clone()
            } else {
                base.join(path)
            }
        } else {
            generate(&mut session, &script.provider, line)?
        };
        let samples = to_pcm24k(&AudioClip::decode(
            fs::read(&source).with_context(|| format!("Cannot read {}", source.display()))?,
        )?)?;
        let path = work.join(format!("line-{:04}.wav", index + 1));
        write_wav(&path, &samples)?;
        prepared.push(Prepared {
            path,
            count: samples.len() as u64,
            line: line.clone(),
        });
    }
    let groups = chapters(
        &prepared,
        script.chapter_seconds,
        script.lead_in,
        script.tail,
    )?;
    let mut chapter_paths = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let audio = work.join(format!("chapter-{:04}.wav", index + 1));
        let timing = compose(
            &audio,
            &prepared[group.clone()],
            if index == 0 { script.lead_in } else { 0.0 },
            if index + 1 == groups.len() {
                script.tail
            } else {
                0.0
            },
        )?;
        let chapter = work.join(format!(
            "chapter-{:04}.{}",
            index + 1,
            output.extension().unwrap().to_string_lossy()
        ));
        println!(
            "Recording chapter {}/{} ({:.2} seconds)",
            index + 1,
            groups.len(),
            timing.duration
        );
        session.send(json!({"op":"audio","path":audio}))?;
        session.send(json!({"op":"record_start","path":chapter,"codec":codec,"fps":script.fps}))?;
        let started = Instant::now();
        let mut last_line = None;
        while started.elapsed().as_secs_f64() < timing.duration {
            let now = started.elapsed().as_secs_f64();
            if let Some((line_index, start, length, pause)) = timing
                .lines
                .iter()
                .enumerate()
                .find_map(|(i, (start, length, pause))| {
                    (now >= *start && now < *start + *length + *pause)
                        .then_some((i, *start, *length, *pause))
                })
            {
                let line = &prepared[group.start + line_index].line;
                if last_line != Some(line_index) {
                    if let Some(emotion) = &line.emotion {
                        session
                            .send(json!({"op":"emotion","name":emotion,"duration":length+pause}))?;
                    }
                    last_line = Some(line_index);
                }
                let fraction = ((now - start) / length).clamp(0.0, 1.0);
                let mut head = line.head_at(fraction);
                if now > start + length && pause > 0.0 {
                    let fade = (1.0 - (now - start - length) / pause).clamp(0.0, 1.0) as f32;
                    for angle in &mut head {
                        *angle *= fade;
                    }
                }
                session.send(json!({"op":"tracking","source":"agent","values":{
                    "yaw":head[0],"pitch":head[1],"roll":head[2]}}))?;
            }
            let next = (now * 20.0).floor() as u64 + 1;
            let target = Duration::from_secs_f64(next as f64 / 20.0);
            if let Some(remaining) = target.checked_sub(started.elapsed()) {
                thread::sleep(remaining);
            }
        }
        session.send(json!({"op":"record_stop"}))?;
        wait_for_export(&mut session, &chapter, Duration::from_secs(600))?;
        chapter_paths.push(chapter);
    }
    if chapter_paths.len() == 1 {
        fs::rename(&chapter_paths[0], &output)?;
    } else {
        concat_chapters(&work, &chapter_paths, &output)?;
    }
    println!("Saved {}", output.display());
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    Ok(parent
        .canonicalize()?
        .join(path.file_name().context("Output needs a filename")?))
}

struct Session {
    socket: TcpStream,
    token: String,
}
impl Session {
    fn connect() -> Result<Self> {
        let port: u16 = env::var("L2D_API_PORT")
            .unwrap_or_else(|_| "4141".into())
            .parse()?;
        let token = if let Some(token) = env::var("L2D_API_TOKEN").ok().filter(|s| !s.is_empty()) {
            token
        } else {
            fs::read_to_string(app_paths::data_dir()?.join("tmp").join(if port == 4141 {
                "api-token".into()
            } else {
                format!("api-token-{port}")
            }))?
            .trim()
            .into()
        };
        let socket = TcpStream::connect_timeout(
            &SocketAddr::from(([127, 0, 0, 1], port)),
            Duration::from_secs(10),
        )?;
        socket.set_read_timeout(Some(Duration::from_secs(10)))?;
        socket.set_write_timeout(Some(Duration::from_secs(10)))?;
        Ok(Self { socket, token })
    }
    fn send(&mut self, mut request: Value) -> Result<Value> {
        request
            .as_object_mut()
            .context("Request must be an object")?
            .insert("token".into(), json!(self.token));
        serde_json::to_writer(&mut self.socket, &request)?;
        self.socket.write_all(b"\n")?;
        let mut line = String::new();
        BufReader::new(&mut self.socket).read_line(&mut line)?;
        ensure!(!line.is_empty(), "Studio closed the agent connection");
        let response: Value = serde_json::from_str(&line)?;
        ensure!(
            response["ok"] == true,
            "{}",
            response["error"]
                .as_str()
                .unwrap_or("Studio request failed")
        );
        Ok(response["result"].clone())
    }
}

fn generate(session: &mut Session, provider: &str, line: &Line) -> Result<PathBuf> {
    let previous = session.send(json!({"op":"status"}))?["audio_path"]
        .as_str()
        .map(str::to_owned);
    let mut command = json!({"op":"tts","provider":provider,"text":line.text,"autoplay":false});
    if let Some(voice) = &line.voice {
        command["voice"] = json!(voice);
    }
    if let Some(model) = &line.tts_model {
        command["model"] = json!(model);
    }
    session.send(command)?;
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        let status = session.send(json!({"op":"status"}))?;
        if status["voice_busy"] == false {
            let path = status["audio_path"]
                .as_str()
                .context("Speech generation did not return audio")?;
            ensure!(
                previous.as_deref() != Some(path),
                "Speech generation failed: {}",
                status["notice"].as_str().unwrap_or("no new audio")
            );
            return Ok(PathBuf::from(path));
        }
    }
    bail!("Speech generation timed out")
}

fn to_pcm24k(clip: &AudioClip) -> Result<Vec<i16>> {
    let mono: Vec<f32> = clip
        .samples
        .chunks_exact(clip.channels as usize)
        .map(|frame| frame.iter().sum::<f32>() / clip.channels as f32)
        .collect();
    let first = mono
        .iter()
        .position(|v| v.abs() > 0.003)
        .context("Audio line is silent")?;
    let last = mono.iter().rposition(|v| v.abs() > 0.003).unwrap();
    let pad = (clip.rate as usize / 25).max(1);
    let mono = &mono[first.saturating_sub(pad)..(last + pad + 1).min(mono.len())];
    let length = ((mono.len() as f64 * RATE as f64 / clip.rate as f64).ceil() as usize).max(1);
    ensure!(
        length <= RATE as usize * 540,
        "One voice line exceeds chapter maximum"
    );
    let mut output = Vec::with_capacity(length);
    for i in 0..length {
        let source = i as f64 * clip.rate as f64 / RATE as f64;
        let index = (source.floor() as usize).min(mono.len() - 1);
        let next = (index + 1).min(mono.len() - 1);
        let fraction = (source - index as f64) as f32;
        let value = mono[index] * (1.0 - fraction) + mono[next] * fraction;
        output.push((value.clamp(-1.0, 1.0) * 32767.0).round() as i16);
    }
    Ok(output)
}
fn write_wav(path: &Path, samples: &[i16]) -> Result<()> {
    let mut file = File::create(path)?;
    wav_header(&mut file, samples.len() as u32)?;
    for chunk in samples.chunks(8192) {
        let mut bytes = Vec::with_capacity(chunk.len() * 2);
        for sample in chunk {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        file.write_all(&bytes)?;
    }
    Ok(())
}
fn wav_header(file: &mut File, count: u32) -> Result<()> {
    let bytes = count.checked_mul(2).context("WAV is too large")?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36u32 + bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&RATE.to_le_bytes())?;
    file.write_all(&(RATE * 2).to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&bytes.to_le_bytes())?;
    Ok(())
}

struct Prepared {
    path: PathBuf,
    count: u64,
    line: Line,
}
fn chapters(
    lines: &[Prepared],
    seconds: f64,
    lead: f64,
    tail: f64,
) -> Result<Vec<std::ops::Range<usize>>> {
    let mut groups = Vec::new();
    let mut start = 0;
    let mut length = lead;
    for (index, line) in lines.iter().enumerate() {
        let item = line.count as f64 / RATE as f64 + line.line.pause;
        ensure!(
            item + tail <= seconds,
            "Line {} is longer than one chapter",
            index + 1
        );
        if index > start && length + item + tail > seconds {
            groups.push(start..index);
            start = index;
            length = 0.0;
        }
        length += item;
    }
    groups.push(start..lines.len());
    Ok(groups)
}
struct Timing {
    duration: f64,
    lines: Vec<(f64, f64, f64)>,
}
fn compose(path: &Path, lines: &[Prepared], lead: f64, tail: f64) -> Result<Timing> {
    let mut file = File::create(path)?;
    wav_header(&mut file, 0)?;
    let mut count = 0u64;
    let mut timing = Vec::new();
    append_silence(&mut file, &mut count, (lead * RATE as f64).round() as u64)?;
    for line in lines {
        timing.push((
            count as f64 / RATE as f64,
            line.count as f64 / RATE as f64,
            line.line.pause,
        ));
        let mut source = File::open(&line.path)?;
        source.seek(SeekFrom::Start(44))?;
        let copied = std::io::copy(&mut source, &mut file)?;
        ensure!(
            copied == line.count * 2,
            "Prepared audio file changed during performance"
        );
        count += line.count;
        append_silence(
            &mut file,
            &mut count,
            (line.line.pause * RATE as f64).round() as u64,
        )?;
    }
    append_silence(&mut file, &mut count, (tail * RATE as f64).round() as u64)?;
    ensure!(count <= u32::MAX as u64 / 2, "Chapter audio is too large");
    file.seek(SeekFrom::Start(0))?;
    wav_header(&mut file, count as u32)?;
    Ok(Timing {
        duration: count as f64 / RATE as f64,
        lines: timing,
    })
}
fn append_silence(file: &mut File, count: &mut u64, samples: u64) -> Result<()> {
    let zeros = [0u8; 8192];
    let mut remaining = samples * 2;
    while remaining > 0 {
        let n = remaining.min(zeros.len() as u64) as usize;
        file.write_all(&zeros[..n])?;
        remaining -= n as u64;
    }
    *count += samples;
    Ok(())
}
fn wait_for_export(session: &mut Session, output: &Path, limit: Duration) -> Result<()> {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(500));
        let state = session.send(json!({"op":"status"}))?;
        match state["recording"]["state"].as_str() {
            Some("saved") => {
                ensure!(
                    output.exists(),
                    "Studio reported saved but output is missing"
                );
                return Ok(());
            }
            Some("failed") => bail!(
                "Recording failed: {}",
                state["recording"]["error"]
                    .as_str()
                    .unwrap_or("unknown error")
            ),
            _ => {}
        }
    }
    bail!("Recording did not finish saving within ten minutes")
}
fn concat_chapters(work: &Path, chapters: &[PathBuf], output: &Path) -> Result<()> {
    let list = work.join("chapters.ffconcat");
    let mut text = String::from("ffconcat version 1.0\n");
    for path in chapters {
        text.push_str(&format!(
            "file {}\n",
            path.file_name().unwrap().to_string_lossy()
        ));
    }
    fs::write(&list, text)?;
    let ffmpeg = env::var_os("FFMPEG").unwrap_or_else(|| "ffmpeg".into());
    let result = Command::new(ffmpeg)
        .current_dir(work)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-n",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            "chapters.ffconcat",
            "-c",
            "copy",
        ])
        .arg(output)
        .stdin(Stdio::null())
        .output()?;
    ensure!(
        result.status.success(),
        "Chapter join failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wav_has_exact_duration_and_no_extra_gap() {
        let dir = tempfile::tempdir().unwrap();
        let line = dir.path().join("line.wav");
        write_wav(&line, &vec![0; RATE as usize]).unwrap();
        let prepared = Prepared {
            path: line,
            count: RATE as u64,
            line: Line {
                text: "Hi".into(),
                voice: None,
                tts_model: None,
                emotion: None,
                pause: 0.18,
                motion: vec![],
                audio: None,
            },
        };
        let result = compose(&dir.path().join("chapter.wav"), &[prepared], 0.0, 0.0).unwrap();
        assert!((result.duration - 1.18).abs() < 0.001);
        assert_eq!(
            fs::metadata(dir.path().join("chapter.wav")).unwrap().len(),
            44 + (result.duration * RATE as f64 * 2.0) as u64
        );
    }
}
