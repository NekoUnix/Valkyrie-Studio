# Valkyrie Studio v0.3.0-alpha.1

The application and control engine are Rust with the MIT-licensed Purism Core C model runtime. The release includes portable packages for Windows x64, macOS Apple Silicon and Intel, Ubuntu x64, Fedora x64, and Arch x64. `valkyrie-studio` loads a local model; `valkyrie-agent` connects an authenticated local agent; `valkyrie-perform` records scripted lines and head movement with short measured gaps.

Windows integration checks used a local 208-parameter, 17-atlas model and completed an 11.8-second two-chapter alpha WebM with audio. CI compilation/tests pass on all six targets, but macOS/Linux GUI and device behavior remains unverified. The release excludes model files, credentials, and FFmpeg. Read [Alpha status](ALPHA_RELEASE.md), [Validation](../VALIDATION.md), and [Licensing](../THIRD_PARTY.md) before using it for a production session.
