# Agent API

[Quick connection](CONNECT_AGENT.md) · [Performance scripts](AGENT_PERFORMANCE.md)

Studio accepts UTF-8 JSON lines on TCP `127.0.0.1:4141`. The protocol is neither HTTP nor WebSocket. Each request needs `token` and `op`, plus an optional `id`; each response has `ok` and either `result` or `error`. The `valkyrie-agent` CLI reads the session token automatically from the per-user `tmp/api-token` file. Set `L2D_API_PORT` and `L2D_API_TOKEN` only for deliberate local test sessions.

```json
{"token":"SESSION_TOKEN","id":1,"op":"emotion","name":"joy","duration":3}
```

The API has a bounded command queue and packet length; high-rate tracking is coalesced by source so slow consumers do not accumulate old poses. Tracking values use `yaw`, `pitch`, `roll` in degrees and ARKit-style blendshapes in 0–1. `schema` returns the loaded model's parameters and drawables. Commands available in this Rust alpha are:

| Operation | Fields | Effect |
|---|---|---|
| `status` | — | Model, mode, guide, voice, recording and performance status; no provider key |
| `schema` | — | Parameter ranges, drawable IDs and model canvas |
| `physics` | optional `settings` object, `settle`: boolean | Read physics groups/settings; replace validated settings or clear momentum |
| `mode` | `mode`: agent/phone/webcam/idle | Select control source |
| `tracking` | `source`, `values` | Send pose/blendshape values |
| `calibrate` | — | Reset neutral-pose calibration |
| `emotion` | `name`: joy/thinking/angry/surprised/neutral, `intensity`, `duration` | Temporary expression |
| `parameters` / `parameters_clear` | Model parameter `values`, `duration` | Set or release direct parameters |
| `view` | `zoom` 0.1–30, `x`/`y` −12–12 | Frame the model |
| `canvas` | even `width`/`height`, 16–8192 | Reallocate output canvas before recording |
| `guides` | `preset`, `enabled`, `mock_ui`, `opacity` 0–0.8, `margins` | Preview-only safe areas and representative platform controls |
| `load_model` | `path` to `.model3.json` | Replace the primary model |
| `snapshot` | new PNG `path` | Save the rendered canvas without guides |
| `ui_settings` | `scale` 0.75–2.5 | Multiply the operating system's UI scale |
| `elevenlabs_refresh_voices` | — | Load accessible account voices |
| `elevenlabs_configure` / `voice_configure` | `settings`, optional `api_key`, `remember` | Set ElevenLabs/OpenAI voice settings |
| `elevenlabs_forget_key` / `voice_forget_key` | — | Remove provider key |
| `tts` | `provider`, `text`, optional `voice`, `model`, `speed`, `instructions`, `autoplay` | Generate typed speech asynchronously |
| `audio` / `audio_play` / `audio_stop` | local audio `path` | Load/play/stop a clip and mouth tracking |
| `record_start` | `path` or `output`, `codec`, `fps` | Start video and loaded audio together |
| `record_stop` | — | Stop capture; poll `status.recording.state` for `saved` or `failed` |

`status.recording` includes `frames`, `captured`, `dropped`, `duplicates`, `output`, and `error`. UDP tracking can listen on editable addresses and ports in the UI; the agent TCP API remains loopback-only. Unsupported legacy operations return an explicit error. Never put session tokens or API keys in a script that will be committed.

`physics` returns the model's group IDs, names, inputs, outputs, particle counts, imported multipliers and current settings. Its `settings` field is a complete `PhysicsSettings` object, so read it first and modify the fields you need before sending it back. `motion_style` is `Bouncy`, `Natural`, or `Authored`; overall settings are `enabled`, `strength` (0–2), `inertia` (0–2), `response` (0.25–2), `gravity` (0–2), `wind` (−1–1), and `groups` keyed by authored group ID. Group objects use the same fields. Edits save per model. `{"op":"physics","settle":true}` resets current momentum.
