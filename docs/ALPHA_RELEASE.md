# v0.3.0-alpha.1 release status

[Home](../README.md) · [User guide](USER_GUIDE.md) · [Validation](../VALIDATION.md)

This release replaces the app and control engine with Rust and retains MIT-licensed Purism Core as the separate C model runtime. The GitHub workflow builds six portable archives from the release tag: Windows x64, macOS Apple Silicon, macOS Intel, Ubuntu x64, Fedora x64, and Arch x64. Each archive contains `valkyrie-studio`, `valkyrie-agent`, and `valkyrie-perform`, plus docs, examples, the cleared video walkthrough, and MIT notices. It contains no model, API key, or FFmpeg executable. The archives are not signed installers.

| Platform | Verified for this alpha |
|---|---|
| Windows x64 | CI compile/tests; local GPU model loading, voice, fullscreen portrait recording, and two-chapter WebM checked on the development PC |
| macOS Apple Silicon / Intel | CI compile/tests and packaging; GUI, audio, capture, and install/startup not yet checked on physical Macs |
| Ubuntu, Fedora, Arch x64 | CI compile/tests and packaging; GUI, audio, capture, and install/startup not yet checked on physical Linux desktops |

The Rust UI covers model loading, view control, UI scaling, social safe-area guides, agent/UDP tracking, typed OpenAI/ElevenLabs speech, voice selection, and video recording. Phone UDP bind/port fields are editable. The webcam adapter starts separately. Attachment/background editing and direct VTube Studio WebSocket connection from the old Ruby UI are not ported. Physics is independently implemented and may differ from the original model author's intended movement. [Validation](../VALIDATION.md) lists measured performance and gaps.

The [81-second Vaelari walkthrough](../media/Valkyrie-Studio-Walkthrough-Public.mp4) shows a scripted agent and ElevenLabs voice with a model moving and speaking together. It predates this Rust renderer; the current Rust proof is a shorter local two-chapter test, not a new full-length promotional video. No still or model files are published. See [licensing and model rights](../THIRD_PARTY.md).
