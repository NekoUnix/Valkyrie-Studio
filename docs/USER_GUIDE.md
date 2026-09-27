# User guide

[Home](../README.md) · [Walkthrough video](../media/Valkyrie-Studio-Walkthrough-Public.mp4) · [Build and packages](BUILD.md)

## Open and frame your model

Start `valkyrie-studio` and paste the full path to a local `.model3.json` into **Model manifest**, then choose **Load model**. The app reads that manifest, its `.moc3`, textures, and supported physics file in their original folder. A standalone `.moc3` is insufficient. You can also launch with `--model PATH`.

The default canvas is 1080×1920. Use **Zoom** (0.1–30) and **Pan X/Y** (−12 to +12) to frame it; a face close-up of the local test model used about 3.3 zoom and −1.6 vertical pan. Press **F11** to toggle fullscreen. **UI scale** changes the controls from 75% to 250% without changing video size. To set a different even canvas size, use the local agent command `{"op":"canvas","width":1920,"height":1080}` before recording.

**Portrait guides** shade likely interface-covered areas for TikTok, Shorts, Reels, Stories, Snapchat, Pinterest, X, LinkedIn, Threads/Bluesky, Twitch vertical clips, and WhatsApp Status. **All platforms** combines the margins. **Custom** lets you edit four sides. These are estimates and appear in preview only; check the current social app before publishing.

## Move the model

Choose **agent**, **phone**, **webcam**, or **idle** above Framing. The active mode receives tracking; the first samples calibrate a neutral pose. Use `valkyrie-agent --connect` to switch to agent mode, verify READY, and send JSON control commands. [Agent connection](CONNECT_AGENT.md) explains how to connect a language-model tool or another local program.

For a phone, run an iFacialMocap-compatible sender that sends ARKit/iFacialMocap UDP data to this computer. The Rust app listens on loopback ports 15483, 8001, and 49983 by default. Open **Phone and webcam tracking**, set the UDP listen address to `0.0.0.0` and enter the desired ports, then choose **Apply listening addresses**. In the phone app, set its destination to this PC's LAN IP and one of those ports (often 49983 for iFacialMocap). Select **phone** mode and verify movement in the preview. Use a trusted private network and allow the selected UDP port through the OS firewall if needed; the authenticated agent TCP port stays on loopback.

For a webcam, install the optional Python/OpenCV/MediaPipe helper described in [Build](BUILD.md), then run it with your camera index or stream URL and a local `face_landmarker.task`. It sends only landmark scores to `127.0.0.1:15483`; select **webcam** mode. The Rust UI does not yet start or configure this helper. `valkyrie-agent '{"op":"calibrate"}'` repeats neutral calibration.

## Speak typed dialogue

Choose **ElevenLabs** or **OpenAI** under Voice. Paste the provider key into the masked field and choose **Use key**. On Windows, **Remember on this Windows account** encrypts it with DPAPI outside the repo; on macOS/Linux, use session entry or the `OPENAI_API_KEY` / `ELEVENLABS_API_KEY` environment variable. **Forget key** removes the saved key for that provider.

For ElevenLabs, choose **Refresh my voices** and select one of the returned account voices. For OpenAI, select one of the listed voices and set speed. Enter text and choose **Speak and animate**. Generation happens on a background thread. Playback drives audio-envelope mouth movement while model physics and other controls continue. **Replay** and **Stop** control that clip. This is typed speech, not an AI microphone conversation. Provider usage may be billed.

## Record

Set an output filename that does not exist. Choose H.264 or H.265 for `.mp4`, VP9 for alpha `.webm`, or ProRes for alpha `.mov`. Install an FFmpeg build with both `utvideo` and the format's encoder; set `FFMPEG` if it is not on PATH. The recorder keeps the preview interactive, uses bounded GPU readback, and saves a lossless intermediate before final compression. At 1080×1920, that intermediate can be hundreds of MB even for a short clip. Leave room and wait for the **saved** state after stopping. The UI shows total, repeated, and dropped frames.

For long reads, use [performance scripts](AGENT_PERFORMANCE.md). The runner generates voice first, trims leading/trailing silence, defaults to a 0.18-second line gap, synchronizes head cues with each measured line, and records chapters. A video demonstration of the workflow is linked above; it predates the Rust renderer.

## Current alpha limits

The Rust app has one primary model. Background image/video editing, accessory attachments, direct VTube Studio WebSocket control, and automatic webcam launch remain unported. Purism physics is an independent approximation and can differ from the original rig author's Cubism Framework motion. Mac and Linux packages pass CI compilation/tests but await hands-on GUI/recording tests. [Validation](../VALIDATION.md) records what has actually been checked.
