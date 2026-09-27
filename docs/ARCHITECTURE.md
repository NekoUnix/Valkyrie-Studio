# Architecture

Valkyrie Studio's UI, control engine, model loader, spring physics, audio playback, voice clients, tracking transport, and export orchestration are Rust. The vendored [Purism Core](../third_party/purism_core/README.md) is a separately licensed MIT C library compiled by Cargo's build script. WGPU renders model meshes into an RGBA canvas; egui displays that canvas and the local controls. No official Cubism SDK is linked or packaged.

A single UI/render thread owns model state and GPU calls. TCP agent workers queue bounded commands; UDP tracking keeps only the latest frame per source. Voice requests run on worker threads. Recording uses bounded GPU readback and a bounded encoder queue; a background FFmpeg process writes a lossless UtVideo capture, then encodes the selected delivery format. Missing capture ticks repeat the last frame to preserve duration and appear as `duplicates` in status. The performance runner first prepares all audio lines, then records synchronized head cues and mouth playback in chapters. Temporary capture files can be large.

The optional `helpers/webcam.py` sends only blendshape and head-pose values to the Rust UDP listener. It is an external camera adapter, not a second application runtime. The older Ruby/C++ desktop code is historical and is not part of the Rust package.

The model's `.model3.json`, `.moc3`, textures, and physics data stay in the user's original folder. No model assets or service credentials are compiled into or packaged with the app. [Validation](../VALIDATION.md) describes tested fidelity and platform limits.
