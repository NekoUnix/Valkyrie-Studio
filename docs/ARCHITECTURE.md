# Architecture and ownership

For using the app, start with the [quick start](../README.md) and
[user guide](USER_GUIDE.md). This page explains how the native renderer, Ruby
controller, tracking inputs, agent API, audio, and encoder fit together for
contributors debugging or extending Valkyrie Studio.

```text
Native webcam helper ─UDP─┐
iPhone / VTS bridge ──UDP─┼─> Ruby Inbox ─> Ruby Engine + Mapper + Calibration
External agents ────TCP──┘                     │
Native UI/drop ─> command queue ────────────────┤
                                              v
                        Ruby main thread: Model / Renderer / Scene
                              │ batched FFI values per rig
                              v
             C++ bridge: Purism Core model evaluation + native spring physics
                              │
                  OpenGL 3.3 Core textures, meshes, masks, FBO
                              ├──> native ImGui/GLFW preview
                              └──> RGBA readback ─> Ruby bounded queue ─> FFmpeg
```

Ruby is the orchestrator. It owns selection, scene references, tracking normalization,
calibration, smoothing, idle/emotion/direct-parameter precedence, timeline scheduling,
API authorization, audio analysis/provider requests and FFmpeg process lifecycle.
The C++ layer owns only native handles, efficient GPU operations, Purism evaluation,
native control widgets, and miniaudio playback. No renderer call originates in a
network or TTS worker. GLFW initialization/event polling remain on the main thread,
including on macOS. FFI callbacks are held by the App object for their full lifetime.

## C ABI and models

`native/bridge.h` is the public ABI. Every model has its own opaque handle, so
parameters and updates cannot accidentally target a global last-loaded model.
Exported `load/update/draw/query` functions translate C++ exceptions into error strings.
`l2d_set_parameters` takes one float buffer in discovered parameter order to avoid
hundreds of Ruby/native transitions per model per frame. `l2d_draw` takes five floats:
`scale_x, scale_y, translate_x, translate_y, rotation_radians`.

The bridge owns aligned Purism moc/model memory and invokes its C API for
consistency, revive, initialize, and update. The independent spring solver reads
the model's `physics3.json`, runs bounded 60 Hz steps, preserves each particle's
rest direction and segment length, and uses soft output limits to prevent a
high-gain chain from folding across the model. The renderer consumes Purism
drawables directly. No official Cubism Core or Framework is linked.

Model assets resolve relative to the model directory. Traversal outside that directory
is rejected. Model files and image inputs are bounded; missing textures, inconsistent
mocs, unsupported extended blends and missing sibling model JSONs produce errors.
Replacing a primary first loads the candidate successfully, then releases the old rig.

## GL pipeline

The renderer uses VAOs, VBOs, index buffers and GLSL 330. Atlas textures stay resident;
the live mesh vertex stream is uploaded each frame. Drawables are stable-sorted by
Cubism render order. Mask groups render alpha into a separate full-resolution FBO;
consecutive identical groups reuse that result. Inverted clipping samples `1-mask`.
Normal/additive/multiply blending follows the traditional Cubism model conventions.
Offscreen and extended blending models are explicitly unsupported.

The scene FBO is premultiplied RGBA8. It is composited into the native preview with
premultiplied blending. Offline/PNG readback uses synchronous CPU row flipping and
alpha conversion. Live capture converts orientation and straight alpha in a GPU shader,
then reads into a ring of three reusable pixel-pack buffers. Zero-timeout fence polls
copy only completed buffers; full rings skip capture instead of waiting for the GPU.
Color at zero alpha is zeroed. Guides/checkerboard belong to UI draw lists and never
enter output. Texture quads rotate in aspect-correct physical canvas space.

ArtMesh anchor pose is a centroid and a stable, long vertex edge. Its current/reference
edge length gives uniform scale and edge angle gives rotation. This follows mesh
motion but is not skinning and may be unsuitable for highly deforming meshes.

## Timing and queues

Offline frame `i` always uses time `i/fps` and `dt=1/fps`; physics and audio analysis
use this clock. Live input timestamps use the app's monotonic timeline. Stale tracking
returns to idle. Smoothing is exponential in time, avoiding frame-rate-dependent gains.
Source neutral samples use a median baseline; expression normalization uses the remaining
0–1 range. No lighting reconstruction is claimed.

Tracking ingress coalesces old UDP frames; the control queue is bounded and rejects
overflow. TCP input/output sizes and clients are bounded. The encoder queue is also
bounded: no unbounded frame accumulation. Live enqueue is nonblocking; a full queue
declines the incoming frame and increments a visible counter. Items carry frame indices.
Only the pipe worker repeats previous images to fill missing time; there is no catch-up
loop on the UI thread. A final target frame count preserves wall-clock duration through
the last capture tick. Offline capture still writes every frame with backpressure.

Live encoding uses lossless Ut Video (`gbrap`) in a Matroska intermediate. Stop closes
the queue and starts a separate finalization thread; it drains capture and encodes the
selected delivery format, then muxes audio. FFmpeg progress reaches UI status. Captures
default to 30 FPS while preview runs at display/render speed. 60 unique frames/second
is not promised for a model whose rendering alone falls below that rate.

FFmpeg stderr is drained concurrently and retained as a bounded diagnostic tail.
Failed or cancelled exports keep `.partial` files; success renames them to the requested
name after the encoder exits successfully. Live intermediates survive a failed final
encode and are removed only on success. Existing final/partial outputs are refused.
The export command uses an argument vector, never a shell string.

## Audio

FFmpeg converts input to bounded 48 kHz mono signed 16-bit PCM. Miniaudio plays that
canonical file; live lip sync samples its playback cursor. Export restarts audio from
zero and muxes the same canonical file. Idle/no-audio has no mouth source. Rhubarb cues
are optional and validated; the fallback is an envelope/zero-crossing proxy, not ASR.

Cloud synthesis and decoding run in a single background preparation job. Results return
to the GL thread through a queue. Keys are environment-only. The studio does not create
or manage external agents, and never includes their LLM credentials.

The video encoder receives only raw frames during capture. Once the exact frame count
is known, a second FFmpeg pass pads/trims canonical audio to `frame_count/fps`.
Offline exports copy their already encoded video; live exports compress the lossless
intermediate to the requested delivery codec. This avoids `-shortest` packet buffering
extending PCM audio beyond the intended video duration.

## Connections and preview guides

The connection controller validates editable settings, rebinds only tracking UDP
listeners (restoring the previous binding on failure), sends iFacialMocap startup
requests and owns webcam/VTS helper processes. Stopping a source disables its packets;
closing the app terminates only helpers it started. The TCP agent service is unaffected.
Live packet counters distinguish a listening socket from actual received tracking.
Local preferences hold connection and guide settings; sources require manual Start.

Social guides are normalized portrait margins plus a schematic UI drawn by ImGui,
after the scene texture. The All platforms profile takes the maximum margin on each
edge. Editing any margin switches to Custom. They do not touch the export FBO and
are intentionally approximate across devices, placements and platform revisions.
