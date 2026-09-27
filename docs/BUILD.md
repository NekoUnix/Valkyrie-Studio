# Build and run

[Home](../README.md) · [User guide](USER_GUIDE.md) · [Validation](../VALIDATION.md)

The [release page](https://github.com/NekoUnix/Valkyrie-Studio/releases) has portable archives for Windows x64, macOS Apple Silicon, macOS Intel, Ubuntu x64, Fedora x64, and Arch x64. CI compiles and tests each on its named runner or container. A CI pass is not a hands-on graphics, camera, or recording test on macOS/Linux.

From source, install Rust 1.98.1, a C compiler, and the desktop libraries for your OS, then run:

```sh
cargo test --locked
cargo build --release --locked
```

On Windows use the MSVC toolchain and Visual Studio C++ Build Tools, or an LLVM-MinGW environment. On macOS install Xcode command-line tools. On Ubuntu use `libasound2-dev libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxi-dev libxcursor-dev libxinerama-dev libgl1-mesa-dev libvulkan-dev pkg-config`. Fedora and Arch dependencies are listed exactly in [.github/workflows/rust-builds.yml](../.github/workflows/rust-builds.yml). The build compiles the vendored MIT Purism Core header into the Rust application; it does not fetch or link the official Cubism SDK.

Launch `target/release/valkyrie-studio` or `valkyrie-studio.exe`. The agent and performance binaries are beside it. Packaging scripts `scripts/package-rust.sh LABEL` and `scripts/package-rust.ps1 -Label LABEL` produce the platform archive in `dist/`. They include all three binaries, docs, the cleared walkthrough, examples, and MIT notices, but no models, credentials, or FFmpeg binary.

For video, install FFmpeg with `utvideo` plus `libx264` (MP4), `libx265` (H.265 MP4), `libvpx-vp9` (transparent WebM), or `prores_ks` (transparent MOV). Set `FFMPEG` to an absolute executable path if needed. The recorder checks encoders before starting and will not overwrite an existing output. A GPU/driver that supports WGPU on the platform is required.

The optional webcam helper is Python, OpenCV, and MediaPipe because the Rust core currently accepts normalized tracking packets rather than owning a camera. Install `helpers/requirements.txt` in a local virtual environment, obtain a `face_landmarker.task` for your own use, and run:

```sh
python helpers/webcam.py --model /path/to/face_landmarker.task --camera 0 --port 15483 --preview
```

Use `--camera` with a numeric index or stream URL; `--backend`, `--width`, `--height`, and `--fps` are editable. Select **webcam** mode in Studio. No model file or landmarker asset is distributed. On Windows, `scripts/webcam.ps1 -Model C:\path\to\face_landmarker.task -Camera 0 -Preview` is a convenience wrapper; pass `-Python` for a virtual-environment interpreter if needed.

Per-user data defaults to `%APPDATA%\ValkyrieStudio` on Windows, `~/Library/Application Support/ValkyrieStudio` on macOS, and `$XDG_DATA_HOME/valkyrie-studio` or `~/.local/share/valkyrie-studio` on Linux. `VALKYRIE_HOME` can override it with an absolute path. The local agent token lives in its `tmp/` directory; Windows-saved provider keys live in `.credentials/` encrypted by DPAPI. Keep both private.
