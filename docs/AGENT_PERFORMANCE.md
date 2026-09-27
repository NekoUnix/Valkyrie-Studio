# Agent voice and motion performances

Start with the [user guide](USER_GUIDE.md) if you have not loaded a model or chosen a
voice yet. The [walkthrough video](../media/Valkyrie-Studio-Walkthrough-Public.mp4)
shows a short agent-controlled Vaelari demonstration with ElevenLabs lip-sync.

An AI agent can write a JSON script with voice lines and head movements, then
render one MP4. Generated audio sets the timing: head cues use a fraction of
each line's **measured** duration, and lip-sync reads those same samples. No
real-time command loop has to keep pace with the renderer.

1. Start Valkyrie Studio and load a `.model3.json`. In **Audio &
   Speech**, connect ElevenLabs or OpenAI and select the voice you want. The
   selected voice is used unless a script line supplies `voice`.
2. Have your AI agent create a JSON file like
   [`examples/vaelari_performance.json`](../examples/vaelari_performance.json).
   Tell the agent the dialogue, emotional beats, and head turns. Do not include
   an API key or session token in the script.
3. In the project folder, validate and render:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/perform.ps1 -ScriptPath examples/vaelari_performance.json -Validate
   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/perform.ps1 -ScriptPath examples/vaelari_performance.json -OutputPath output/Vaelari-Performance.mp4
   ```

On macOS/Linux, run `ruby bin/agent_perform --script SCRIPT --output VIDEO.mp4`
after installing the Ruby dependencies. The Windows launcher uses this
workstation's bundled Ruby and FFmpeg when available.

The CLI connects to the running studio through its authenticated local API.
The studio generates each line with the selected provider key; the CLI reads
only the resulting audio file. It builds audio and motion timelines per chapter,
runs the native offline renderer, and joins the chapters into an H.264/AAC MP4.
The adjacent `.performance` folder retains chapter WAVs, motion timelines,
timing metadata, and render reports. Choose a **new** output filename per run.
Quiet padding at the edges of generated lines is trimmed with a small safety
margin; breaths and pauses inside a line are kept. The default added gap is
0.18 seconds between lines, and chapter boundaries add no extra hold.

| Script field | Meaning |
|---|---|
| `provider` | `elevenlabs` (default), `openai`, or `system` |
| `model_path` | Optional avatar path; otherwise uses the model loaded in the app |
| `width`, `height`, `fps` | Even canvas dimensions and 24, 30, or 60 FPS |
| `background` | Opaque RGBA color for MP4 |
| `view` | Model framing: `zoom` 0.1–30; `x` and `y` −12 to +12 |
| `chapter_seconds` | Maximum audio length per chapter; default 180, maximum 540 |
| `lead_in`, `tail` | Seconds held before and after each chapter |
| `lines` | 1–1000 ordered voice lines |

Each line has `text` (1–4000 characters), optional `voice` (provider voice ID),
`tts_model`, `emotion`, `pause`, and `motion`. A motion cue has `at` from 0.0
(speech begins) to 1.0 (speech ends), plus optional `yaw`, `pitch`, and `roll`
angles in degrees. Cues interpolate smoothly; unspecified angles are zero.
Without cues, the head gets gentle automatic movement and blinks. Audio controls
mouth motion; head cues intentionally omit jaw and mouth channels. Split very
long paragraphs into lines with natural pauses.

This is a scripted agent performance. A live language model can create the JSON
and call the CLI as a tool, but the renderer never invents dialogue. The studio
must stay open through voice preparation; rendering then runs in a separate
hidden native window in the same desktop session. Avoid starting a live
recording during preparation.
