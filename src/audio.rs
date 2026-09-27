//! Decoded playback and mouth envelopes share one sample clock.
use crate::tracking::Values;
use anyhow::{Context, Result, ensure};
use rodio::{Source, buffer::SamplesBuffer};
use std::{
    io::Cursor,
    num::{NonZeroU16, NonZeroU32},
    time::Duration,
};

pub struct AudioClip {
    pub samples: Vec<f32>,
    pub channels: u16,
    pub rate: u32,
    pub duration: f64,
    envelopes: Vec<(f32, f32)>,
}
impl AudioClip {
    pub fn decode(bytes: Vec<u8>) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= 64 * 1024 * 1024,
            "Audio input must be 1–64 MiB"
        );
        let decoder =
            rodio::Decoder::try_from(Cursor::new(bytes)).context("Cannot decode WAV/MP3 audio")?;
        let channels = decoder.channels().get();
        let rate = decoder.sample_rate().get();
        ensure!(
            (1..=2).contains(&channels) && rate <= 192_000,
            "Use mono/stereo audio at up to 192 kHz"
        );
        let maximum = (rate as usize * channels as usize * 600).min(64 * 1024 * 1024);
        let samples: Vec<f32> = decoder.take(maximum + 1).collect();
        ensure!(
            !samples.is_empty()
                && samples.len() <= maximum
                && samples.iter().all(|v| v.is_finite()),
            "Audio is empty, too long, or exceeds 256 MiB decoded"
        );
        let duration = samples.len() as f64 / (rate as f64 * channels as f64);
        let window = (rate as usize / 100).max(1) * channels as usize;
        let mut envelopes = Vec::with_capacity(samples.len().div_ceil(window));
        for chunk in samples.chunks(window) {
            let mono: Vec<f32> = chunk
                .chunks(channels as usize)
                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                .collect();
            let rms = (mono.iter().map(|v| v * v).sum::<f32>() / mono.len() as f32).sqrt();
            let crossings = mono
                .windows(2)
                .filter(|s| s[0].is_sign_negative() != s[1].is_sign_negative())
                .count() as f32
                / mono.len() as f32;
            envelopes.push((
                (rms * 5.0).clamp(0.0, 1.0),
                ((crossings - 0.07) * 8.0).clamp(-0.7, 0.7),
            ));
        }
        Ok(Self {
            samples,
            channels,
            rate,
            duration,
            envelopes,
        })
    }
    pub fn mouth(&self, time: f64) -> Values {
        if time < 0.0 || time > self.duration {
            return Values::from([
                ("ParamMouthOpenY".into(), 0.0),
                ("ParamMouthForm".into(), 0.0),
            ]);
        }
        let index = time * 100.0;
        let a = self
            .envelopes
            .get(index.floor() as usize)
            .copied()
            .unwrap_or((0.0, 0.0));
        let b = self
            .envelopes
            .get(index.floor() as usize + 1)
            .copied()
            .unwrap_or(a);
        let t = index.fract() as f32;
        Values::from([
            ("ParamMouthOpenY".into(), a.0 + (b.0 - a.0) * t),
            ("ParamMouthForm".into(), a.1 + (b.1 - a.1) * t),
        ])
    }
    fn buffer(&self) -> SamplesBuffer {
        SamplesBuffer::new(
            NonZeroU16::new(self.channels).unwrap(),
            NonZeroU32::new(self.rate).unwrap(),
            self.samples.clone(),
        )
    }
}

pub struct AudioPlayer {
    stream: rodio::MixerDeviceSink,
    player: Option<rodio::Player>,
}
impl AudioPlayer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            stream: rodio::DeviceSinkBuilder::open_default_sink()
                .context("Audio output unavailable")?,
            player: None,
        })
    }
    pub fn play(&mut self, clip: &AudioClip) {
        self.stop();
        let player = rodio::Player::connect_new(self.stream.mixer());
        player.append(clip.buffer());
        self.player = Some(player);
    }
    pub fn stop(&mut self) {
        if let Some(player) = self.player.take() {
            player.stop();
        }
    }
    pub fn position(&self) -> Option<Duration> {
        self.player
            .as_ref()
            .filter(|p| !p.empty())
            .map(rodio::Player::get_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_audio_rejected_without_device() {
        assert!(AudioClip::decode(vec![0; 1024]).is_err());
    }
}
