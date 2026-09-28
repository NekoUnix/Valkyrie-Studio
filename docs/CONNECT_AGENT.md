# Connect an AI agent

[Home](../README.md) · [API reference](API.md) · [Long performances](AGENT_PERFORMANCE.md)

1. Start Valkyrie Studio and load your local `.model3.json`.
2. In a terminal beside the packaged binaries, run `valkyrie-agent --connect` (`valkyrie-agent.exe` on Windows). It selects **agent** mode and should print **READY**. `--check` reports status without changing mode.
3. Try `valkyrie-agent '{"op":"emotion","name":"joy","duration":3}'`. The model should react. Set stronger continuous movement with `valkyrie-agent '{"op":"puppet","energy":1.5}'`, then try `valkyrie-agent '{"op":"gesture","name":"nod","duration":0.8}'`. In **Inputs**, the same Motion energy slider and gesture buttons let you preview the effect. Try `{"op":"view","zoom":3.3,"y":-1.6}` for a close-up, or `{"op":"tracking","source":"agent","values":{"yaw":10,"pitch":2}}` to turn the head.
4. Give your AI agent access to the local CLI as an allowed tool, or implement the authenticated JSON-lines TCP protocol in [API](API.md). The agent decides the dialogue and commands; Studio renders them. A scripted demo should be described as scripted, not as live language-model inference.

The app binds the agent API to `127.0.0.1:4141`. The CLI reads the random session token from the app's per-user data directory. Keep that token, provider keys, and model files out of prompts and commits. For smooth motion, reuse one TCP connection and send tracking updates around 20–30 times per second; each command receives one JSON response. Use `schema` to discover the loaded model's parameter IDs.

Automatic agent movement adds small head and body sway, gaze, and blinks between commands. `puppet.energy` ranges from 0 (off) to 2 (strongest); 1 is the normal live setting. `gesture` accepts `nod`, `shake`, `tilt`, or `lean`, with optional `intensity` 0–2 and `duration` 0.2–5 seconds. Gestures layer over tracking, so an agent can continue streaming a pose while the avatar reacts. Explicit eye tracking and audio-driven mouth movement keep their own control.

To have the agent speak, configure ElevenLabs in the app and send `{"op":"tts","provider":"elevenlabs","model":"eleven_v3","text":"Hello from Vaelari!","emotion":"joy","intensity":2}`. With autoplay, the matching facial expression and gesture begin with audio playback, and the mouth follows the sound. Wait until `status.voice_busy` is false before another TTS request. For long scripts, the separate `valkyrie-perform` command prepares and times all lines first, so the avatar can speak and move together without generation pauses inside the recording. Its [example](../examples/vaelari_performance.json) pairs line emotion, voice, gesture, and head motion.
