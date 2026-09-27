# Alpha release status

[Home](../README.md) · [User guide](USER_GUIDE.md) · [Validation](../VALIDATION.md)

Valkyrie Studio v0.2.0-alpha.1 is a **public source-only alpha**. This repository contains MIT-licensed Ruby/C++ application source, documentation, examples, tests, and the redacted 81-second [Vaelari walkthrough](../media/Valkyrie-Studio-Walkthrough-Public.mp4). Its active renderer uses MIT-licensed Purism Core. It does not contain any model, provider keys, downloaded tools, or an installer. The walkthrough predates the Purism migration.

## Platform availability

| Platform | What is prepared | What remains before a downloadable app |
|---|---|---|
| Windows x64 | Purism source build and native bridge tested with local models | Portable dependency packaging and clean-machine test |
| Linux x86_64 | Build instructions and CMake source | Build and run on Linux; test GL, FFmpeg, and dependencies |
| macOS Intel | Build instructions and architecture-specific CMake source | Build and run on Intel Mac; package and sign if applicable |
| macOS Apple Silicon | Build instructions and architecture-specific CMake source | Build and run on Apple Silicon; package and sign if applicable |

The repository source archive can be downloaded on any of these platforms, but it is **not** a prebuilt app for each platform. Do not label source archives as native installers. Read the [new release notes](RELEASE_NOTES_0.2.0-alpha.1.md) before building.

## What the alpha demonstrates

- Load a Live2D .model3.json and use phone, webcam, or agent tracking.
- Choose a typed OpenAI or ElevenLabs voice; select voices from the ElevenLabs account.
- Preview 9:16 social framing and scale the UI without changing export dimensions.
- Record opaque MP4 or transparent WebM/MOV; render agent-written dialogue and motion as synchronized chapters.
- Use zoom to 12× and pan ±3 in the UI; agent scripts can use zoom to 30× and pan ±12.

The [walkthrough](../media/Valkyrie-Studio-Walkthrough-Public.mp4) is a scripted local agent demonstration made with the earlier renderer, with the model moving and speaking simultaneously. It is 1920×1080, 30 FPS, 81 seconds, H.264/AAC. Local network addresses have been hidden. The original footage remains local under output/ and is not intended for GitHub.

## Release boundaries

The active build does not use Live2D's official SDK, so the prior SDK-based publication requirement is not asserted for this build. This does **not** establish that every licensing question is resolved: any distributed model needs its own permission, Purism's MIT notice must travel with binaries, and bundled tools have separate terms. The user cleared video footage, not model-file distribution. See [Third-party components](../THIRD_PARTY.md). No platform binary should be labeled ready until packaging and target-platform checks pass.

The native runtime also needs actual Linux and macOS builds and tests before platform-specific downloads can be offered honestly. The detailed current evidence and limitations are in [Validation](../VALIDATION.md).
