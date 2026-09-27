# Native agent API

For a first-time connection, follow [Connect an AI agent](CONNECT_AGENT.md). You
usually do not need to open a socket yourself: with Studio running and a model
loaded, `ruby bin/agent --connect` selects agent mode and reports **READY**. Then
try `ruby bin/agent '{"op":"emotion","name":"joy","duration":3}'`.

The rest of this page is for agent/tool authors who need the protocol and complete
command list. For the buttons and recording workflow, use the [user guide](USER_GUIDE.md).

Connect to `127.0.0.1:4141` using a TCP socket. Send **one UTF-8 JSON object plus a
newline per request**. This port is TCP JSON, not HTTP or WebSocket. Every request
contains `token`, `op`, and optionally your `id`. Get the session token from
`tmp/api-token`, or set `L2D_API_TOKEN` in `.env`. A blank token configuration causes
the studio to generate a random token on each launch.

```json
{"id":"turn-1","token":"YOUR_TOKEN","op":"emotion","name":"joy","intensity":0.8,"duration":3}
```

Responses preserve your ID:

```json
{"ok":true,"result":true,"id":"turn-1"}
{"ok":false,"error":"Unknown emotion","id":"turn-1"}
```

Network validation failures may omit `id`. Errors never include provider API keys.
Eight clients, 64 KiB input buffers, 16 pending requests/client, 256 KiB output
buffers and 128 queued control operations bound memory use. Slow clients time out
after 120 seconds of inactivity. Tracking UDP is latest-frame-wins per source;
control requests are acknowledged only after the render thread applies them.

| `op` | Other fields | Result / behavior |
|---|---|---|
| `status` | — | Current model and local `model_path`, loaded audio duration and path, canvas, controls, notice/error, attachment list |
| `schema` | — | Full discovered parameter ranges/defaults, drawable IDs and canvas metadata |
| `mode` | `mode`: agent/phone/webcam/idle | Select source; restart neutral calibration |
| `tracking` | `source` (default agent), `values` object | ARKit blendshapes and yaw/pitch/roll; input expires after 0.5 s |
| `calibrate` | — | Sample a fresh neutral pose |
| `emotion` | `name`: joy/thinking/angry/surprised/neutral, `intensity` 0–1, `duration` 0–600 | Temporary expression/head preset |
| `parameters` | `values` object, `duration` 0–600 | Overlay actual model parameters, validated and clamped; per-ID expiry |
| `parameters_clear` | — | Release all manual/direct overrides |
| `canvas` | even `width`, `height`, each 16–8192 | Allocate exact output FBO; forbidden while recording |
| `view` | `zoom` 0.1–30, `x`,`y` -12–12 | Model framing in normalized canvas coordinates. Studio sliders support zoom 0.1–12 and pan -3–3. |
| `load_model` | `path`, `primary` boolean, optional `x`,`y` | Primary or anchored secondary rig; model loading happens on GL thread |
| `prop` | PNG `path`, `x`,`y` -4–4 | Prop at normalized canvas coordinates, anchored to nearest visible ArtMesh |
| `attachment` | `index`, optional `scale`, `rotation` radians, `anchor` index | Modify selected attachment; anchor -1 detaches |
| `attachment_remove` | `index` | Release prop/rig resources |
| `background` | `color` [r,g,b,a] **or** image/MP4 `path` | Clear color, image texture or looping video |
| `audio` | `path`, optional Rhubarb JSON `visemes` | Async preparation in live mode; status reports completion |
| `audio_play` | — | Start loaded clip from zero |
| `audio_stop` | — | Stop loaded clip and release mouth tracking |
| `tts` | `provider`: openai/elevenlabs/system (default openai), `text`, optional `voice`, `model`, `speed`, `instructions`, `autoplay` | Async TTS; OpenAI and ElevenLabs use their saved voice settings and can automatically play/lip-sync the result |
| `voice_configure` | `settings`: model/voice/speed/instructions/autoplay; optional `api_key`, `remember`, `persist` | Update OpenAI configuration; remembered Windows keys are DPAPI-encrypted separately from preferences |
| `voice_forget_key` | — | Remove the stored key and disconnect OpenAI for this session |
| `elevenlabs_configure` | `settings`: voice_id/model/autoplay; optional `api_key`, `remember`, `persist` | Save ElevenLabs settings and an optional encrypted Windows key; a supplied key triggers an asynchronous voice refresh |
| `elevenlabs_refresh_voices` | — | Fetch accessible account voices across pages without blocking rendering |
| `elevenlabs_forget_key` | — | Remove the stored ElevenLabs key and disconnect this session |
| `ui_settings` | `scale` 0.75–2.5, `follow_dpi` boolean, optional `persist` | Rebuild fonts and scale controls without changing export dimensions |
| `record_start` | `output`, `codec`: h264/h265/prores/vp9, optional `fps` (default 30 live) | Start lossless intermediate capture and audio from zero |
| `record_stop` | — | Immediately return `{state: "saving", output: ...}` in live mode; poll `status.export.state` for `saved` or `failed` |
| `connection_start` | `source`: phone/webcam, `settings` object, optional `persist` (default true) | Apply connection fields, start source and select its mode |
| `connection_stop` | `source`: phone/webcam | Stop the owned helper/packet ingestion and return selected source to idle |
| `guides` | `preset`, optional `enabled`, `mock_ui`, `opacity` 0–0.8, `margins` [left,top,right,bottom], `persist` | Update preview-only portrait guides; custom margins are fractions 0–0.45 |
| `ui_tab` | `tab`: Scene/Inputs/Guides/Audio | Select controls or the audio sidebar in compact layouts |
| `ui_snapshot` | new `.png` `path` | Schedule native window screenshot including preview guides |
| `snapshot` | new `.png` `path` | Read and save the current canvas (no guides) |
| `quit` | — | Acknowledge then shut down gracefully |

Coordinate origin is the canvas center: left/right = -1/+1, bottom/top = -1/+1.
Zoom and pan affect model framing; prop cursor positions are inverted through that
same transform before choosing an ArtMesh. Secondary rigs can be loaded from `.moc3`
only if their sibling `.model3.json` and textures exist.

Emotion/tracking values are normalized source controls. `parameters` uses the
model's actual parameter units and IDs, discovered by `schema`. Bone coordinates
are represented as model angle/parameter values; these 2D rigs have no 3D bone skeleton.

`status.export` contains `state`, `frames`, `captured`, `queue`, `dropped`, `duplicates`,
`output` and final compression `progress` (0–1). `missed_capture_ticks` tracks render/GPU
misses separately from encoder queue drops. `record_fps` is independent of preview
`fps`. Duration is preserved by repeated images; repeats are not unique captured frames.

`status.voice` contains public voice configuration, `configured`, `busy` and a connection
message, never the API key. `configured` means a key is available; successful speech
generation is the actual access test. `status.ui_settings` exposes scale/DPI preferences.
Tests on ephemeral API ports write `tmp/api-token-PORT`, leaving the main studio's
`tmp/api-token` undisturbed.

`connection_settings` exposes the editable fields, and `connections` reports per-source
packet count, age, peer, live status and helper state. Settings keys are `phone_protocol`
(`ifacial`, `udp`, `vts`), `phone_ip`, `phone_port`, `listen_bind`, `listen_port`, `vts_url`,
`webcam_source`, `webcam_backend` (`auto`, `dshow`, `msmf`, `v4l2`, `avfoundation`),
`webcam_width`, `webcam_height`, `webcam_fps`, `webcam_port` and `webcam_preview`.
Keep any IP-camera credentials local: connection settings are local plaintext and are
available to authenticated API clients. `persist:false` is useful for automated checks.

## Python agent example

```python
import json, pathlib, socket
token = pathlib.Path('tmp/api-token').read_text().strip()
with socket.create_connection(('127.0.0.1', 4141)) as sock:
    stream = sock.makefile('rwb')
    request = {'token': token, 'id': 1, 'op': 'emotion', 'name': 'joy', 'duration': 3}
    stream.write((json.dumps(request) + '\n').encode())
    stream.flush()
    print(json.loads(stream.readline()))
```

## Offline timeline

```json
{
  "duration": 2,
  "events": [
    {"time": 0, "command": {"op": "emotion", "name": "joy", "duration": 1}},
    {"time": 1, "command": {"op": "parameters", "values": {"ParamAngleX": -15}, "duration": 1}}
  ]
}
```

Events are stable-sorted by time then source order and execute at the first frame
whose simulation timestamp reaches the event time. Offline rendering accepts no
live tracking/network events. Load primary model/audio through CLI before export.
Prepare speech before rendering; cloud TTS is rejected inside an offline timeline.
For multi-line speech, `bin/agent_perform` prepares and renders synchronized
chapters; see [Agent performance](AGENT_PERFORMANCE.md).

The API can read local assets and write requested output files. Keep it loopback
bound; changing its bind address to a LAN interface is an explicit operator decision.
No remote shell operation is provided. All subprocess calls use argument arrays.
