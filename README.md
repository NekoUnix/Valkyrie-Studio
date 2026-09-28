# Valkyrie Studio

Valkyrie Studio is a Rust desktop app for loading a local `.model3.json`, moving and voicing the avatar, and recording video. It uses the MIT-licensed Purism Core `.moc3` runtime; the official Live2D Cubism SDK is not built or shipped. Bring your own model and FFmpeg. No model assets or API keys are in this repository or the release packages.

[Watch the full-screen Vaelari walkthrough](media/Valkyrie-Studio-Walkthrough-Public.mp4). The 81-second video shows a scripted agent driving the model and ElevenLabs speech. It was recorded with an earlier renderer, so it demonstrates the workflow rather than the exact appearance of this Rust build.

## Get started

1. Download the package for your platform from [Releases](https://github.com/NekoUnix/Valkyrie-Studio/releases). Windows and macOS each have their own package; Ubuntu, Fedora, and Arch have separate Linux builds. These are portable alpha archives, not installers or signed apps.
2. Extract it. Install FFmpeg with `utvideo` and the encoder for your desired output: `libx264` for MP4, `libvpx-vp9` for transparent WebM, or `prores_ks` for transparent MOV. Put `ffmpeg` on `PATH` or set `FFMPEG` to its full path.
3. Start `valkyrie-studio` (`valkyrie-studio.exe` on Windows). In **Stage**, select **Browse models…** and choose your local `.model3.json`. The app remembers recent models and reopens the last available one next time. Keep its referenced `.moc3` and textures in place.
4. Choose **Vertical 9:16** or **Horizontal 16:9** in **Stage** or **Capture**, then frame with **Zoom** and **Pan**. Press **F11** for fullscreen. The controls follow desktop display scaling; **UI size** adds a 75–250% adjustment. For vertical shots, pick a social platform to preview its safe area and representative chat, action, caption, and navigation controls.
5. In **Voice**, enter an OpenAI or ElevenLabs key in the masked field, select a voice, type text, and select **Speak and animate**. The app explains this flow beside the button. On Windows you can save the key encrypted for your user account; on macOS/Linux keys remain session-only or can come from environment variables.
6. In **Capture**, choose a new `.mp4`, `.webm`, or `.mov` output path and matching codec, then select **Start recording** and **Stop recording**. Wait for **saved**. Export runs in the background and may need substantial temporary disk space.

The [user guide](docs/USER_GUIDE.md) covers the controls and current alpha limits. The [build guide](docs/BUILD.md) covers source builds. [Validation](VALIDATION.md) separates measured Windows behavior from CI compilation on the other platforms.

## Connect an agent

Keep Studio running with a model loaded. In the extracted package, run `valkyrie-agent --connect` (`.exe` on Windows). It should report **READY**. The CLI reads the session token from the app's per-user data folder and connects only to `127.0.0.1:4141`. An agent can send one JSON command at a time, for example:

```sh
valkyrie-agent '{"op":"emotion","name":"joy","duration":3}'
valkyrie-agent '{"op":"puppet","energy":1.5}'
valkyrie-agent '{"op":"gesture","name":"nod","duration":0.8}'
```

In agent mode, the model now keeps moving between commands with gentle head and body sway, eye movement, and blinking. The **Inputs** tab has a **Motion energy** slider and gesture preview buttons; `puppet` accepts 0–2 (1 is normal, 0 disables automatic movement). An agent can still send precise tracking at 20–30 Hz, and `gesture` adds a short nod, shake, tilt, or lean without replacing that tracking. Speech continues to drive the mouth.

For a voiced performance with stronger head movement, copy and edit [the JSON example](examples/vaelari_performance.json), then run `valkyrie-perform --script performance.json --output take.webm`. The runner prepares each voice line, measures the audio, aligns mouth movement and 20 Hz head cues, records bounded chapters, and joins them. Set `motion_energy` and optional line `gesture` values to direct its movement. The default pause between lines is 0.18 seconds; adjust each line's `pause` if needed. See [agent connection](docs/CONNECT_AGENT.md), [performance scripts](docs/AGENT_PERFORMANCE.md), and the [API](docs/API.md).

## Current alpha scope

The Rust app provides the Purism renderer, A.R.I.A-derived particle physics with motion styles and per-group tuning, agent/UDP tracking with editable listening addresses, typed OpenAI and ElevenLabs speech, a voice picker, portrait safe-area guides, scalable UI, and H.264/H.265 MP4, VP9 WebM, or ProRes MOV recording. Model rendering and audio playback were tested on Windows with a local Vaelari model. CI compiles and tests the six build targets, but macOS and Linux GUI, camera, FFmpeg, and package startup still need physical target-machine tests. Phone tracking uses a UDP sender; the optional Python webcam helper is started separately. The older Ruby interface's attachment/background editing and direct VTube Studio API connection have not been ported. See [alpha status](docs/ALPHA_RELEASE.md).

The app is [MIT licensed](LICENSE); [Purism Core's MIT notice](third_party/purism_core/LICENSE) is included. Model rights, FFmpeg build terms, and other dependency notices are separate. See [third-party components](THIRD_PARTY.md). The cleared walkthrough is a video permission, not permission to redistribute the model files.
