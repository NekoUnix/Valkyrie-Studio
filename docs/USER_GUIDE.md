# User guide

[Home](../README.md) · [Walkthrough video](../media/Valkyrie-Studio-Walkthrough-Public.mp4) · [Build and packages](BUILD.md)

## Open and frame your model

Start `valkyrie-studio`, open **Stage**, choose **Browse models…**, and select a local `.model3.json`. You can also use **Load path**, drag the manifest onto the window, or launch with `--model PATH`. Recent models appear in Stage and the last available model reopens at startup. The app reads its `.moc3`, textures, physics file, and optional matching `.vtube.json` from their original folder. A standalone `.moc3` is insufficient.

The default canvas is 1080×1920. Choose **Horizontal 16:9** for 1920×1080, or **Vertical 9:16** to switch back; both buttons are in Stage and Capture. The preview and recording use the selected canvas. Use **Zoom** (0.1–30) and **Pan X/Y** (−12 to +12) to frame it; a face close-up of the local test model used about 3.3 zoom and −1.6 vertical pan. Press **F11** to toggle fullscreen. The UI follows your desktop DPI; **UI size** multiplies that scale from 75% to 250% without changing video size. **Use desktop size** resets the multiplier. For other even canvas sizes, use the local agent `canvas` command before recording.

**Social preview** shades likely interface-covered areas for TikTok, Shorts, Reels, Stories, Snapchat, Pinterest, X, LinkedIn, Threads/Bluesky, Twitch vertical clips, and WhatsApp Status. It also shows representative action icons, chat/reply controls, captions, and navigation. Toggle **Show platform controls, chat and captions** if you want just the safe area; **Shade** changes its opacity. **All platforms** combines the margins, and **Custom** lets you edit four sides. These estimates appear in preview only and never enter recordings; check the current social app before publishing.

## Tune physics

Open **Physics** after loading a model with a referenced `.physics3.json`. **Bouncy** gives the longest rebound; **Natural** settles sooner; **Authored** keeps A.R.I.A's earlier solver at the file's authored step rate. The enhanced styles use 120 fixed steps per second with interpolated tracking inputs and outputs. Pause recovery drops stale motion after a long frame, and the chain keeps authored segment lengths while bounding velocity. This is an independent solver, not exact VTube Studio or Cubism Framework physics.

The overall sliders set strength, inertia, response speed, gravity, and wind. Use group search to find tail, hair, ear, clothing or accessory chains by name or parameter. Expand a group to enable or disable it, tune those same properties, inspect its authored inputs/outputs, or reset it. **Settle motion** clears momentum. Group actions enable, disable or reset all groups. Settings save per model in the user's Valkyrie data folder; the avatar files stay untouched. A matching VTube Studio sidecar, if present, contributes its enabled state, global strength/wind, and group multipliers as the initial baseline. Tail groups start with gentler inertia on a new profile; you can change them freely.

## Move the model

Choose **agent**, **phone**, **webcam**, or **idle** in **Inputs**. The active mode receives tracking; the first samples calibrate a neutral pose. Use `valkyrie-agent --connect` to switch to agent mode, verify READY, and send JSON control commands. [Agent connection](CONNECT_AGENT.md) explains how to connect a language-model tool or another local program.

In agent mode, **Motion energy** sets how much head sway, body movement, eye motion, and blinking continue between commands. Start at 1, try 1.5 for a more animated presentation, or set 0 to turn automatic motion off. The **nod**, **shake**, **tilt**, and **lean** buttons preview short gestures; an agent can send the same actions through the local API while continuing to drive head tracking and speech. The model's available parameter ranges limit the result, so some rigs may move less than others.

For a phone, run an iFacialMocap-compatible sender that sends ARKit/iFacialMocap UDP data to this computer. The Rust app listens on loopback ports 15483, 8001, and 49983 by default. Open **Inputs**, set the UDP listen address to `0.0.0.0` and enter the desired ports, then choose **Apply listening addresses**. In the phone app, set its destination to this PC's LAN IP and one of those ports (often 49983 for iFacialMocap). Select **phone** mode and verify movement in the preview. Use a trusted private network and allow the selected UDP port through the OS firewall if needed; the authenticated agent TCP port stays on loopback.

For a webcam, install the optional Python/OpenCV/MediaPipe helper described in [Build](BUILD.md), then run it with your camera index or stream URL and a local `face_landmarker.task`. It sends only landmark scores to `127.0.0.1:15483`; select **webcam** mode. The Rust UI does not yet start or configure this helper. `valkyrie-agent '{"op":"calibrate"}'` repeats neutral calibration.

## Speak typed dialogue

Open **Voice** and choose **ElevenLabs** or **OpenAI**. Paste the provider key into the masked field and choose **Use key**. On Windows, **Remember on this Windows account** encrypts it with DPAPI outside the repo; on macOS/Linux, use session entry or the `OPENAI_API_KEY` / `ELEVENLABS_API_KEY` environment variable. **Forget key** removes the saved key for that provider.

For ElevenLabs, choose **Refresh my voices** and select one of the returned account voices. For OpenAI, select one of the listed voices and set speed. Enter text and choose **Speak and animate**. The app's Voice tab explains the sequence: it generates audio on a background thread, plays the clip, and drives audio-envelope mouth movement while model physics and agent motion continue. **Replay** and **Stop** control that clip. This is typed speech, not an AI microphone conversation. Provider usage may be billed.

## Record

Open **Capture** and set an output filename that does not exist. Choose H.264 or H.265 for `.mp4`, VP9 for alpha `.webm`, or ProRes for alpha `.mov`. Install an FFmpeg build with both `utvideo` and the format's encoder; set `FFMPEG` if it is not on PATH. The recorder keeps the preview interactive, uses bounded GPU readback, and saves a lossless intermediate before final compression. At 1080×1920, that intermediate can be hundreds of MB even for a short clip. Leave room and wait for the **saved** state after stopping. The UI shows total, repeated, and dropped frames.

For long reads, use [performance scripts](AGENT_PERFORMANCE.md). The runner generates voice first, trims leading/trailing silence, defaults to a 0.18-second line gap, synchronizes head cues with each measured line, and records chapters. Use `motion_energy` and optional line `gesture` fields for stronger puppeteering. A video demonstration of the workflow is linked above; it predates the Rust renderer.

## Current alpha limits

The Rust app has one primary model. Background image/video editing, accessory attachments, direct VTube Studio WebSocket control, and automatic webcam launch remain unported. The A.R.I.A-derived physics solver is independent and can differ from the rig author's Cubism Framework motion. Mac and Linux packages pass CI compilation/tests but await hands-on GUI/recording tests. [Validation](../VALIDATION.md) records what has actually been checked.
