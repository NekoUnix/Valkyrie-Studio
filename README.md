# Valkyrie Studio

Valkyrie Studio lets you load a Live2D model, drive it with a phone, webcam, or local AI agent, give it a voice, and record finished video. The interface is native Ruby/C++ with OpenGL; cloud voice services are optional.

[Watch or download the full-screen walkthrough](media/Valkyrie-Studio-Walkthrough-Public.mp4). It shows Vaelari moving under agent control, ElevenLabs speech and lip-sync, social framing guides, and the recording controls. The demonstration uses a scripted agent performance and was recorded before the Purism renderer migration. Local network addresses in the recording have been hidden.

## Start using it

**On the development workstation:** double-click Launch-Valkyrie-Studio.cmd, then load your .model3.json in **Scene** or drop it onto the preview. The launcher uses locally installed tools when available. It does not include a model or provider credentials.

**From a fresh source checkout:** install Ruby 3.2–3.4, Bundler, CMake, a C++17 compiler, and FFmpeg. An OpenGL 3.3 Core GPU/driver is required. The build fetches the pinned [MIT-licensed Purism Core](https://github.com/SakuraMotion/PurismCore) source; no official Cubism SDK is needed. Then follow the [platform build guide](docs/BUILD.md). Models, FFmpeg, and voice-provider keys are not included in this repository.

| Platform | Current state |
|---|---|
| Windows x64 | Native bridge built and the supplied test model exercised on this workstation. Source build instructions are provided. |
| Linux x86_64 | Source build path provided; native runtime has not been tested on Linux hardware. |
| macOS Intel / Apple Silicon | Separate source build paths provided; native runtime has not been tested on either macOS architecture. |

This is a **public alpha source release** for Windows, Linux, and macOS source builds. There are no verified cross-platform installers or bundled model files. See [validation and limitations](VALIDATION.md) before relying on it for a production session.

## Your first session

1. Open Valkyrie Studio and load a .model3.json in **Scene**. The model assets stay in their original folder.
2. Choose a canvas shape: landscape, portrait, square, or custom even dimensions. Adjust **Zoom** and **Pan** to frame the model. The sliders reach 12× zoom and ±3 pan; the agent API supports 30× and ±12.
3. For a vertical video, open **Guides** and choose a social platform or **All platforms**. Guides are preview-only and never appear in the export.
4. In **Inputs**, choose **agent**, **phone**, or **webcam**. Configure the connection there. If using a phone or webcam, hold a neutral pose during the initial calibration.
5. In **Audio & Speech**, load an audio file or choose OpenAI, ElevenLabs, or system speech. Enter typed dialogue, select a voice, and generate speech. Playback drives lip-sync.
6. Choose an output filename and format in **Export**. Press **Record**, then **Finish recording**. The app saves the finished file in the background and reports progress.

For a complete explanation of each control, phone/webcam setup, voice selection, transparent output, and troubleshooting, read the [user guide](docs/USER_GUIDE.md).

## Let an AI agent control Vaelari

With a model visible, open **Inputs → Connect an AI agent** and choose **Enable agent control**. In the project folder run:

~~~sh
ruby bin/agent --connect
ruby bin/agent '{"op":"emotion","name":"joy","duration":3}'
~~~

The first command should report **READY**. Your agent can then use the local CLI or authenticated TCP API to send movement, expressions, scene changes, and speech. For a long narrated video, it can write a JSON script whose voice lines and head movements stay synchronized with measured speech.

See [connect an agent](docs/CONNECT_AGENT.md), [make a narrated performance](docs/AGENT_PERFORMANCE.md), and the [API reference](docs/API.md). Keep the local session token and voice-provider keys private.

## What you can make

Valkyrie Studio supports live H.264/H.265 MP4 recording, transparent VP9 WebM or ProRes 4444 MOV, and frame-complete offline renders. It can add image or looping-video backgrounds and PNG accessories. The 9:16 preview includes guides for TikTok, YouTube Shorts, Instagram/Facebook Reels and Stories, Snapchat, Pinterest, X, LinkedIn, Threads/Bluesky, Twitch vertical clips, and WhatsApp Status. Guide margins are editable estimates, not official platform screenshots.

During live recording, the preview stays responsive while GPU readback and video encoding run in the background. A 30 FPS capture target worked well with the supplied dense test model on this workstation; 60 FPS can repeat frames when rendering cannot keep up. See the [measured results](VALIDATION.md).

## Help and project details

| Need | Read |
|---|---|
| Controls, tracking, voice, recording | [User guide](docs/USER_GUIDE.md) |
| Windows, Linux, macOS source builds | [Build guide](docs/BUILD.md) |
| Connect a local AI agent | [Agent connection](docs/CONNECT_AGENT.md) |
| Long voice and motion scripts | [Agent performances](docs/AGENT_PERFORMANCE.md) |
| Command reference | [API](docs/API.md) |
| Test results and known gaps | [Validation](VALIDATION.md) |
| Alpha release status by platform | [Alpha release status](docs/ALPHA_RELEASE.md) |
| Implementation | [Architecture](docs/ARCHITECTURE.md) |
| Dependencies and distribution terms | [Third-party components](THIRD_PARTY.md) |

The application source is [MIT-licensed](LICENSE). The active renderer uses MIT-licensed Purism Core and an independently written model/physics adapter. The repository excludes model assets, downloaded tools, generated media other than the cleared walkthrough, local settings, and credentials. Model permissions and the terms of other dependencies still apply. See [Third-party components](THIRD_PARTY.md). This release supplies source, not platform binaries.
