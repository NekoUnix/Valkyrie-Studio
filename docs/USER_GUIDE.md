# User guide

[Home](../README.md) · [Watch the walkthrough](../media/Valkyrie-Studio-Walkthrough-Public.mp4) · [Build from source](BUILD.md)

Valkyrie Studio is a desktop workspace for posing, tracking, voicing, and recording a Live2D model. Start with a .model3.json file and choose one control source. You can switch sources later.

## Load and frame a model

Open **Scene → Load primary**, choose a .model3.json, or drop it on the preview. A bare .moc3 needs a neighboring model3 JSON and its referenced textures. The first model is primary; additional dropped model files become accessory rigs. Valkyrie Studio reads the original model files and does not change them.

Choose landscape, 9:16 portrait, square, or custom even canvas dimensions. The canvas dimensions determine the saved video's size, independently of your monitor or interface scale. **Zoom** reaches 0.1–12× in the UI. **Pan** reaches −3 to +3 on each axis. The agent API allows zoom up to 30× and pan ±12 for scripted shots. A large pan may move the model completely out of view; bring x/y back toward zero if the canvas looks empty.

Use **Settings** or **A− / A+** to scale the interface from 75% to 250%. Following display DPI is optional. On small windows, switch between **Controls** and **Audio / Export**; scroll inside each panel to reach the remaining controls. These settings do not alter the video resolution.

Drop a PNG onto the preview to add a prop at the pointer location. In **Attachments**, adjust scale or rotation, choose an ArtMesh to follow, or set anchor −1 to detach it. The prop follows a mesh as a rigid image; it does not deform with the mesh.

## Plan a vertical video

Select a 9:16 canvas, open **Guides**, and pick TikTok, YouTube Shorts, Instagram/Facebook Reels or Stories, Snapchat, Pinterest, X, LinkedIn, Threads/Bluesky, Twitch vertical clips, WhatsApp Status, or **All platforms**. The shaded margins and interface mockup show where controls and captions may cover content. **All platforms** combines the more restrictive margins. You can change all four margins manually.

Keep the model's face and important text inside the highlighted area. These overlays are composition estimates and may vary with app version, device, captions, and ads. They only appear in the preview; recording and snapshots of the canvas omit them.

## Choose how the model moves

Open **Inputs** and choose one active source:

- **Agent:** Click **Enable agent control**. The panel should show **READY** after a model is loaded. Follow [Connect an AI agent](CONNECT_AGENT.md) to test a command, then let an agent drive expressions, tracking, speech, or a long scripted performance.
- **Phone:** Choose iFacialMocap, ARKit UDP, or VTube Studio API. Fill in the fields for your phone or sender, press **Connect phone**, and watch the packet count/receiving indicator. For iFacialMocap, the usual phone/listen port is 49983. Set the phone destination to this PC's address on the same network. For VTube Studio, enable its Plugin API, connect to its WebSocket URL (usually ws://127.0.0.1:8001), and approve the plugin in VTube Studio.
- **Webcam:** Enter a camera index such as 0 or an OpenCV-compatible stream URL. Choose a backend, dimensions, and FPS; press **Start webcam**. The optional preview appears only if requested. The camera helper requires Python, OpenCV, MediaPipe, and its face-landmarker model; see [Build and optional components](BUILD.md).

Hold a neutral face for the first 45 received phone/webcam samples. **Recalibrate neutral** repeats this step. **Stop** ends the selected connection and returns to idle. Devices do not start automatically when the app opens.

If the phone receives no packets, check its destination IP/port, the PC's network interface, and the Windows private-network firewall rule for that UDP port. Virtual adapters may appear in the PC address list. The agent API remains local even if you make a tracking listener available on the LAN.

## Give the model a voice

In **Audio & Speech**, load a WAV/MP3 file or choose a speech provider:

- **ElevenLabs:** Open **ElevenLabs connection**, enter your key in the masked field, and choose **Save key & load voices**. Select from **My voices**. **Refresh my voices** updates the list. Type a line and choose **Generate speech** or **Preview voice**. ElevenLabs usage may consume account credits.
- **OpenAI:** Open **OpenAI voice setup**, enter the key, choose a voice, and optionally set style, speed, or speech model. Type dialogue and generate speech. This is typed text-to-speech, not a microphone conversation. API usage may be billed.
- **System speech:** Uses an installed OS voice and needs no cloud key.

**Play generated speech with lip-sync** loads and plays the result. The model's mouth follows the audio envelope; optional Rhubarb visemes provide more explicit phonetic shapes. Voice generation runs separately from the preview, so the UI should remain responsive.

On Windows, **Remember on this Windows account** encrypts a provider key with DPAPI. Session-only entry avoids saving it. On macOS/Linux, use session entry or a local .env/environment variable. Never put a key in a performance script, Git commit, or agent prompt. **Forget key** removes the stored copy for that provider.

## Record and export

Choose an output path ending in the correct extension:

| Format | Use it for |
|---|---|
| H.264 or H.265 MP4 | Ordinary opaque video for editing or sharing |
| VP9 WebM | Transparent video for overlays and OBS |
| ProRes 4444 MOV | Transparent editing master |

H.264/H.265 need an opaque background. Clear the background for transparency with WebM or ProRes. You can also load an image or looping video as a background. An existing output file is never overwritten; choose a new name for another take.

Set **Record FPS** to 30 for a reliable starting point with dense models. Press **Record**, perform, then **Finish recording**. The UI immediately returns while the final encode continues in the background. Watch the save state before closing. The Export panel reports captured, repeated, missed, and dropped frames. Repeated frames preserve video duration when rendering is slower than the selected FPS, but they are not new animation frames. A lossless intermediate can be large; keep enough free disk space until final compression succeeds.

For a scripted video with many lines, use [Agent voice and motion performances](AGENT_PERFORMANCE.md). It measures the generated speech first, then renders head movement and lip-sync against that timing, avoiding long pauses at chapter boundaries.

## If something goes wrong

| Symptom | Check |
|---|---|
| Empty preview | Load .model3.json, reset pan near 0, and reduce zoom. Confirm the model has its referenced textures. |
| Agent says not ready | Keep Studio open, load a model, select agent mode, then run ruby bin/agent --connect. |
| Phone does not move model | Confirm protocol, sender destination, listen address/port, packet count, and neutral calibration. |
| Webcam does not start | Install helper requirements and face-landmarker model; inspect tmp/webcam-connection.log. |
| Voice list is empty | Save the provider key locally, refresh voices, and check provider status; do not paste the key into logs. |
| Recording has repeated frames | Lower Record FPS, reduce canvas size, or simplify the scene. Preview FPS and unique capture FPS are different. |
| Preview slows while recording | Start at 30 FPS, close other GPU-heavy programs, then reduce canvas size. Finish recording and wait for the background save before closing Studio. A 60 FPS setting cannot create unique frames beyond the model's render rate. |
| Hair or tails move unlike VTube Studio | The Purism build uses an independent spring solver and can differ from the rig author's original setup. Test a short clip first. For diagnosis, launch with `VALKYRIE_DISABLE_PHYSICS=1` to compare the unmodified pose; this removes secondary spring motion, not tracking. |
| Transparent export looks opaque | Use WebM/ProRes, clear the background, and inspect with a player that decodes alpha. |

For build errors, see [Build from source](BUILD.md). For measured limits and untested devices, see [Validation](../VALIDATION.md).
