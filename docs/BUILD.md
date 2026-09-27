# Build Valkyrie Studio from source

[Home](../README.md) · [User guide](USER_GUIDE.md) · [Validation](../VALIDATION.md)

Build the native bridge on the computer where you intend to run the Studio. Windows x64 has been exercised with local models; Linux and macOS instructions are source build paths awaiting hardware validation. The build fetches a pinned [Purism Core](https://github.com/SakuraMotion/PurismCore) revision. It does **not** need the official Cubism SDK. The Purism notice is included in [licenses/PurismCore-LICENSE.txt](../licenses/PurismCore-LICENSE.txt).

Install Ruby 3.2–3.4 with Bundler, CMake 3.24+, a C/C++17 compiler, FFmpeg with the encoders you need, and an OpenGL 3.3 Core GPU/driver. Keep the Ruby, toolchain, and native build on the same architecture. Network access is needed for the first CMake dependency fetch, or set `PURISM_CORE_SOURCE_DIR` to an existing local Purism checkout. A hidden render still needs a desktop GL context.

## Windows x64

For Visual Studio Build Tools 2022, install Desktop development with C++, RubyInstaller x64 with Devkit, CMake, and FFmpeg. In an x64 Native Tools prompt:

~~~bat
scripts\compile.bat
bundle exec ruby bin\studio --model "C:\Models\avatar.model3.json"
~~~

For LLVM-MinGW, put Clang, `mingw32-make`, and CMake on PATH. This workstation also has local copies under ignored `.tools/`:

~~~powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/compile-mingw.ps1
bundle install
bundle exec ruby bin/studio --model 'C:\Models\avatar.model3.json'
~~~

Use separate build directories when switching between MSVC and MinGW. The bridge, GLFW, and `PurismCore.dll` must sit together in `build/bin` on Windows. The project launcher uses local tools when present and otherwise uses PATH.

## macOS Intel or Apple Silicon

Install Xcode command-line tools, Homebrew Ruby/CMake/FFmpeg, and Bundler. Build separately for Intel and Apple Silicon; no universal binary is currently produced.

~~~sh
xcode-select --install
brew install ruby cmake ffmpeg
export PATH="$(brew --prefix ruby)/bin:$PATH"
gem install bundler
sh scripts/compile.sh
bundle exec ruby bin/studio --model "$HOME/Models/avatar.model3.json"
~~~

Run Studio on the main desktop thread. macOS camera permissions are handled by the OS. This path has not been executed on a Mac yet.

## Linux x86_64

For Ubuntu/Debian, install the desktop GL/X11 packages, Ruby, CMake, and FFmpeg. Install Ruby 3.2+ separately if your distribution version is older.

~~~sh
sudo apt-get install build-essential cmake ruby-full ruby-dev libffi-dev \
  libgl1-mesa-dev libx11-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev \
  libwayland-dev libxkbcommon-dev wayland-protocols ffmpeg espeak-ng
gem install bundler
sh scripts/compile.sh
bundle exec ruby bin/studio --model "$HOME/Models/avatar.model3.json"
~~~

For an X11-only build, pass `-DGLFW_BUILD_WAYLAND=OFF` to `scripts/compile.sh`. Linux runtime behavior remains unverified on target hardware.

## Webcam helper and checks

The optional webcam helper needs Python 3.11 or 3.12 and its face-landmarker asset:

~~~sh
python -m venv .venv
# Activate .venv for your shell, then:
pip install -r helpers/requirements.txt
bundle exec ruby scripts/setup_webcam.rb
~~~

The Inputs tab starts the helper only when requested. To run the Ruby suite:

~~~sh
bundle exec ruby -Itest -e 'Dir.glob("test/test_*.rb").each { |file| require_relative file }'
~~~

Set `FFMPEG` and `FFPROBE` to absolute executable paths if they are not on PATH. `VALKYRIE_DISABLE_PHYSICS=1` is a local diagnostic switch for comparing a model's unmodified parameter pose with the native spring solver. See [Validation](../VALIDATION.md) for tested configurations.

| Directory | Purpose |
|---|---|
| `bin/` | Studio, agent client, performance renderer, VTube Studio bridge |
| `lib/live2d_studio/` | Ruby state, tracking, audio, export, app loop |
| `native/` | Purism adapter, spring solver, OpenGL renderer, ImGui controls |
| `helpers/` | Optional OpenCV/MediaPipe webcam helper |
| `scripts/` | Build, launch, and validation utilities |
| `test/` | Ruby tests |
| `examples/` | Editable timelines and agent scripts |

Model files, downloaded dependencies, local tools, build results, credentials, and routine output media remain outside Git. Do not add models to a source archive or release package.
