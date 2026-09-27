# Validation — alpha source preview

## Current Purism build (2026-09-27)

The active Windows x64 bridge builds against pinned MIT-licensed Purism Core,
without official Cubism SDK or Framework sources. Import inspection found
`PurismCore.dll`, GLFW, and system DLLs; no official Cubism Core dependency.
The supplied Vaelari model loads **208 parameters, 1,174 drawables, 17 textures,
and 62 physics settings**. A native integration run completed at least 71 operations,
including recording. The updated 60 Hz spring solver rendered the local Vaelari,
Akari, Hiyori, and Wanko rigs for 60 frames each at 30 FPS without process errors.
These test assets remain outside Git.

The integration MP4 decoded as 320×568 H.264 video plus AAC audio at 0.600 s.
Two equivalent 2-second Vaelari renders at 30 and 60 FPS finished successfully;
their last frames measured 41.72 dB average PSNR. This is evidence of similar
results across frame rates, not identical motion or a perceptual quality score.
At 1080×1920, a five-second 30 FPS VP9 alpha recording averaged 60.17 FPS before
capture and 49.63 FPS during capture. It saved 171 frames (170 captured, one
repeat), with zero queue drops. Stop responded in 0.092 s and final encoding
finished 7.99 s later. The decoded first frame retained an alpha range of
0–255. This short check does not establish sustained throughput.
Evidence is local in `output/live-benchmark-20260927-024138/` and is not a
repository asset.

The renderer now binds its OpenGL program, vertex array, transform, and sampler
uniforms once per model pass instead of once per drawable. A matched 300-frame
720×1280 offline run fell from 6.44 to 6.05 seconds (about 6%); an identical
60-frame screenshot had infinite PSNR, meaning every decoded RGBA pixel matched.
A second five-second 1080×1920 alpha run averaged 54.82 preview FPS during capture
versus 49.63 in the prior run. It saved 170 unique frames, repeated two, and
dropped none; decoded alpha still ranged 0–255. These short runs are affected
by background load and do not establish a stable speedup across hardware.
The local CMake install test contained the bridge, Purism Core, GLFW, and
Purism's MIT notice; it contained no official Cubism DLL or model asset.
The Ruby suite in this restricted runner completed 31 tests and 164 assertions
with five skips and one Windows DPAPI credential-store error; that error is
environment-dependent and unrelated to the renderer. Earlier normal-user
DPAPI checks passed, but the full suite has not been rerun outside this runner
for the Purism revision.

The spring solver is independently written and is **not** a pixel or motion match
for the official Framework physics. It keeps segment lengths and rest directions,
uses bounded fixed steps and soft output limits, and avoids the visibly folded
tail seen in an earlier migration build. A final 30/60 FPS comparison, long
recording soak, physical phone/webcam tests, and a multi-model visual review
are still needed before claiming production quality or superiority to Live2D.
The existing walkthrough and older benchmark results below used the earlier
official-SDK build and are historical evidence only.

## Earlier official-SDK build (historical)

Windows x64 is the only platform on which the native Studio has been built and
exercised with the supplied Live2D model. The [81-second full-screen walkthrough](media/Valkyrie-Studio-Walkthrough-Public.mp4)
shows agent-driven movement and ElevenLabs lip-sync; local network addresses are
hidden in the shareable copy. Linux and macOS have source build instructions but
no tested native binaries or installers. Physical phone/webcam tracking, sustained
long recording, and live OpenAI speech remain unverified.

The current native UI rebuild passed, including the expanded pan/zoom controls.
The current Ruby suite passed **31 tests, 168 assertions**, with no failures or
errors and five environment/integration skips under the normal Windows account.
Earlier measurements below are retained as historical snapshots, not a claim
that every check was rerun for this documentation update.

## Agent connection and full-screen Vaelari walkthrough

- The Inputs tab now presents a visible four-step agent setup, READY state, command
  count and last-command age. `docs/CONNECT_AGENT.md`, `AGENTS.md`, `bin/agent
  --connect/--check` and `examples/agent_performance.rb` give both a user and an
  external AI agent a complete local connection path. On the loaded test model,
  `--connect` reported READY and the example increased accepted control commands
  from 523 to 548 in a two-second check. The scripted demonstration is an
  AI-authored API performance, not a claim of live language-model inference.
- The UI bridge rebuilt successfully. The Ruby suite passed **27 tests, 146
  assertions**, zero failures/errors, with five expected external/platform skips
  under the normal Windows user account. The app displayed 23 ElevenLabs account
  voices. ElevenLabs generated Vaelari's spoken line, and the locally bundled
  FFmpeg fallback decoded its MP3 for native playback/lip-sync without an error.
- A maximized 3840×2120 studio view was captured to 1920×1080 at 30 FPS. The
  walkthrough shows the Scene, combined social guides, the visible agent setup,
  a 16:9 face close-up moving under agent commands, the ElevenLabs voice picker
  and generation, and the closing Scene. The editable Tesseract project packages
  the full-screen footage and separate narration/live-speech layers. Visual
  filmstrips and encoded frames were inspected. Audio presence, duration and
  levels were measured; a human auditory review is still recommended.

## Valkyrie Studio renderer and branding update

- The window, native header, launcher and VTube Studio plugin label now say
  **Valkyrie Studio**. The old Odette launcher remains as a forwarding alias.
- On the 1,174-drawable Odette model, two matched 600-frame hidden runs took
  **12.35/11.79 s** with the previous bridge and **7.95/7.98 s** with the corrected
  renderer. Total wall time fell about **33–36%**. Shader locations and mesh storage
  are reused; unchanged render order is not resorted; redundant framebuffer setup
  is avoided.
- The original and updated 1080x1920 RGBA PNGs were **pixel identical** at both
  the first frame and frame 120, covering animated render-order changes.
  The GPU capture check matched orientation, color and straight alpha with zero byte
  difference. A 30 FPS transparent live capture saved 208 frames with 206 captured,
  two repeats and zero encoder queue drops; preview averaged **50.25 FPS** while
  recording. The decoded WebM first frame retained transparent and partial-alpha
  pixels. These short runs do not establish sustained 60 FPS recording.
- The full media-enabled Ruby suite passed **27 tests, 185 assertions**, no skips.
  Native UI screenshots passed at 100–250% scale with the new name and working
  ElevenLabs voice picker.
  Evidence is in `output/perf-old-frame.png`, `output/perf-fixed-frame.png`,
  `output/perf-old-later.png`, `output/perf-new-later.png`,
  `output/live-benchmark-20260926-215152/` and `output/voice-ui-20260926-215234/`.

## OpenAI voice setup and scalable native UI

- Built and inspected the interface at 100%, 150%, 200% and 250%, with display-DPI
  scaling enabled on this 175%-DPI display. Font atlases are rebuilt, widgets and panels
  scale together, and compact layouts switch between Controls and Audio / Export.
  Scrollbars keep long panels accessible; social mockup text stays proportional to
  the canvas. Native screenshots/API checks: `output/voice-ui-20260926-210404/`.
- Display default: 100% of system DPI, with a 16px base system font. The old interface
  used an unscaled 13px font. Preferences are saved independently of export dimensions.
- Ruby suite: **25 tests, 170 assertions**, no failures/errors/skips. Voice fixtures
  verify the OpenAI endpoint/header, selected model/voice/speed/style, legacy-model
  validation, key exclusion from status/preferences and session-key behavior.
- Windows DPAPI storage passed encryption/decryption/removal with a generated dummy
  key under the actual Windows user profile. The sandbox account cannot access DPAPI
  profile storage; the normal user-session check passed.
- OpenAI's masked setup field, voice selector, preview, asynchronous generation and
  automatic playback are implemented. A live OpenAI speech request remains unverified.

## ElevenLabs account connection and voice picker

- The supplied ElevenLabs key was saved in Windows DPAPI storage outside source and
  preferences. The live API fetched **23 accessible account voices** and selected a
  voice ID persisted in `user-settings.json`; status exposes names/IDs but no key.
- A short live ElevenLabs request generated an **18,016-byte, 1.068-second MP3**. The
  studio decoded it, loaded it into native audio, and reported no error. The visible
  ElevenLabs provider and selected voice were inspected with the Odette model at
  `output/elevenlabs-connected.png`.
- The updated Ruby suite passed **27 tests, 146 assertions**, with no failures/errors
  and five expected skips for platform or external integration checks. The native UI
  build and Odette screenshot checks passed at 100–250% scale.

## Recording and connection update

The initial short export checks did **not** establish usable full-resolution live
alpha capture. A user report of a frozen interface and about 7 FPS exposed synchronous
readback, UI-thread encoder backpressure and a UI-thread catch-up loop. Those paths
have been replaced for live recording by GPU alpha/orientation conversion, three
PBOs with zero-timeout fence polling, nonblocking bounded enqueue and background
lossless capture/final delivery encoding. Offline rendering remains frame-complete.

Measured with the original Odette rig, transparent 1080×1920 canvas and native UI:

| Test | Preview before capture | Preview during capture | Output frames | Stop response | Final WebM save |
|---|---:|---:|---|---:|---:|
| 60 FPS target, ~15.87 s | 55.55 FPS | 46.23 FPS | 952 total / 730 captured / 222 repeats | 0.092 s | 36.09 s |
| 30 FPS target, 30.90 s | 54.24 FPS | 50.51 FPS | 927 total / 925 captured / 2 repeats | 0.096 s | 39.79 s |

Neither run had an encoder-queue drop. Maximum sampled API latency was 0.111 s.
The **30 FPS default** produces essentially complete capture with this model; the
60 FPS selection cannot create unique frames faster than the scene renders.
Final compression runs after Stop, so its duration is separate from capture speed.
These are short measured runs on this workstation, not a long-session or hardware-wide guarantee.

Evidence: `output/live-benchmark-20260926-194949/results.json` and
`output/live-benchmark-20260926-195308/results.json`. Native UI screenshots show the
new editable Inputs tab and TikTok/combined portrait guide previews. The 30 FPS WebM
was decoded with libvpx-vp9: 1080×1920, 927 frames, alpha range 0–255, 1,698,085 fully
transparent pixels and 12,440 partially transparent pixels in frame one. Guides are
absent from the decoded export.

- Expanded Ruby suite: **21 tests, 145 assertions**, no failures/errors/skips. Includes
  deliberate encoder stalls with nonblocking enqueue, exact duration/repeated frames,
  intermediate-to-WebM alpha preservation, editable connection validation, loopback
  phone start requests, received tracking, failed-bind rollback and guide intersections.
- `scripts/verify_capture.rb`: GPU capture matches synchronous reference **byte for byte**
  for color, orientation and alpha; frame index, bounded-ring behavior and reset passed.
- Expanded native integration exercised 70 successful operations in
  `output/integration-20260926-195932`: every guide preset, phone connection/start packet,
  received tracking/status, source stop, webcam helper startup through the app with a
  generated video source (six frames processed), helper failure reporting at end-of-file, and asynchronous
  H.264 saving with audio. Physical cameras and actual phones were not used.
- Studio and helper processes started by the checks were closed after validation.

The original build/export evidence below is retained separately from these live tests.

## Confirmed on this Windows x64 workstation

- Built the native bridge with LLVM-MinGW Clang 23.1.2 and CMake 4.4.3.
- Ruby 3.4.11, native FFI 1.17.4; dependency lockfile includes Windows, Linux and both
  macOS architectures. Portable tooling is project-local, not a system installation.
- Downloaded official Cubism Native SDK 5-r.5. Both SDK-enabled and SDK-free diagnostics
  configurations compiled. The diagnostics build rendered an alpha FBO successfully.
- Created a GLFW OpenGL 3.3 Core context, loaded the original Odette model read-only,
  and rendered through official Cubism Core + Framework physics and the custom Core renderer.
- Discovered **208 model parameters, 1,174 ArtMeshes, 17 texture atlases** and physics.
- Visually inspected the actual model frame and the native studio UI, including
  transparent checkerboard preview and framing guides.
- Ruby tests: **17 tests, 71 assertions, zero failures/errors/skips** with real FFmpeg
  integration enabled. Includes all 52 ARKit fields, documented iFacialMocap aliases,
  negative head angles, malformed packets, calibration, standard/advanced parameter
  fallback, stale tracking, direct overrides, deterministic event order, authentication,
  fragmented TCP input, queue bounds, real audio decode, Rhubarb cue mapping, codecs,
  decoded alpha pixels and exact PCM trim duration.
- Native application integration: **27 successful operations** in
  `output/integration-20260926-192058/results.json`. Exercises schema, expressions,
  direct parameters, portrait/landscape/square FBO allocation and screenshots, PNG
  anchoring/rotation, a secondary Haru rig, attachment removal, asynchronous audio
  preparation, native playback, looping MP4 background, live H.264 recording/mux,
  audio stop and acknowledged graceful shutdown.
- Confirmed native miniaudio load/playback and a playback cursor advancing to 0.3 s.
  This is a device/API check, not an auditory quality assessment.
- Windows System.Speech generated `output/system-speech.wav` using the installed
  voice in the user session. Sandboxed service-account voice access is insufficient;
  ordinary user-session launch works.
- OpenCV 4.14.0 + MediaPipe 0.10.35 installed in `.venv`; the official face landmarker
  loaded and processed a blank frame successfully on CPU. No camera was opened.

## Native model export evidence

| Artifact | Verified properties |
|---|---|
| `output/Odette-1080x1920-Alpha.webm` | VP9, 1080×1920, 90 frames at 30 fps, 3.000 s, alpha metadata |
| `output/odette-performance-alpha.webm` | VP9, 540×960, 180 frames at 30 fps, 6.000 s; decoded frame retains alpha |
| `output/odette-speech-verified.mov` | ProRes 4444, 540×960, 150 frames, 5.000 s video **and** 5.000 s PCM audio |
| `output/integration-20260926-192058/recording.mp4` | Native live H.264 + audio; video and audio durations both 0.583333 s |
| integration `canvas-*.png` | Actual model at 1920×1080, 1080×1080 and 1080×1920 |

Media unit tests encode and decode **H.264, H.265, ProRes 4444 and VP9 alpha**.
The alpha roundtrip tests check transparent, partially transparent and opaque pixels,
not only metadata. VP9 decode explicitly selects `libvpx-vp9`, since some decoders
ignore WebM's auxiliary alpha data.

The first combined integration run exposed a Ruby VM lock starvation issue during
long native calls. Model load/evaluation/draw/readback and audio load now release the
GVL; their native GL work stays on the main thread. The expanded integration passed
after the fix. An audio duration test also caught FFmpeg `-shortest` packet extension;
audio is now muxed/trimmed against the completed video frame count without re-encoding video.

## Agent voice and motion video pipeline

- The new agent performance script validates line text, voice choice, chapter
  length, and normalized head cues. Generated audio sample counts determine
  each line start; 30 Ruby tests passed with 165 assertions and 5 expected skips.
  The Windows `scripts/perform.ps1` launcher validated the three-line example
  using the bundled Ruby and FFmpeg without changing the user's shell PATH.
  The native UI rebuilt successfully with the long-performance guide in Inputs,
  and the relaunched studio reports the test model and agent as READY.
- An isolated running studio used the supplied Odette-named test model and the
  account's selected ElevenLabs voice to synthesize two Vaelari lines. The
  native offline render produced
  `output/Vaelari-Agent-Performance-Demo-20260926.mp4` at 640×360, 24 FPS,
  7.667 seconds, H.264/AAC. Sampled frames show different head angles and
  mouth shapes during the spoken lines.
- A separate two-chapter render joined to
  `output/Vaelari-Agent-Chapter-Check-20260926.mp4` at 10.438 seconds. The
  measured quiet span over its chapter seam was 0.353 seconds, including the
  default 0.18-second line gap and the voice clip's remaining quiet edge.
  No added chapter hold is present. Audio was measured, not auditioned.

## Not yet verified / release requirements

- macOS arm64/x86_64 and Linux runtime builds on real target systems; MSVC build here.
  Source and architecture-specific CMake selection are supplied, not claimed tested.
- Actual physical webcam tracking quality, iPhone network transmission, VTube Studio
  authorization and live VBridger routing. Parser fixtures and CPU inference are tested.
- A live OpenAI paid request remains unverified. Rhubarb cue parsing is tested; an external
  Rhubarb executable was not installed/run.
- OS drag/drop through physical mouse gestures; callback binding, drop-coordinate math,
  prop creation and attachment operations are implemented. Native API equivalents passed.
- Long-session memory/performance soak, high-DPI/Retina execution, real device hotplug,
  installer packaging/code signing, GPU-driver matrix and sustained 1080p60 recording.
- Cubism 5.3 offscreen/group blending and extended blend modes are **unsupported** and
  rejected explicitly. Arbitrary rig conventions cannot all be inferred from parameter IDs.
- Attachment transforms approximate a rigid prop following a deforming ArtMesh; props
  do not themselves deform. Automatic calibration handles neutral offsets, not lighting
  reconstruction or missing tracking signals.
- The earlier official-SDK build did not have Expandable Application publication
  approval. The current Purism build has separate model-asset and third-party
  notice obligations described in [Third-party components](THIRD_PARTY.md).

The checkout is a working Windows-native engineering preview. Production readiness
requires the remaining platform/device/release validation above.
