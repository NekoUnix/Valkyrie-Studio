# Agent connection to Valkyrie Studio

When the user asks you to control the on-screen Live2D model, read
`docs/CONNECT_AGENT.md` and `docs/API.md`. Valkyrie Studio must be running with
a `.model3.json` loaded. Run `ruby bin/agent --connect` from this folder and
verify it reports READY before sending movement. For a visible first action,
run `ruby bin/agent '{"op":"emotion","name":"joy","duration":3}'`.
In demonstrations, the on-screen persona is **Vaelari**; Odette is only the
filename of the supplied test-model asset. Use Vaelari in spoken introductions.

Use `ruby bin/agent 'JSON_COMMAND'` for occasional actions. For smooth motion,
keep one authenticated TCP socket open and send tracking at 20–30 Hz, reading
the response each time. Discover model-specific parameter IDs with `schema`.
The CLI reads `tmp/api-token` automatically; do not copy the token into your
answer, logs, code, or prompts. Never read or reveal provider credentials.
ElevenLabs TTS is available through `tts` after the user configures a voice in
the app. Tell the user when a demonstration is scripted rather than driven by
a live language-model response.

For narrated videos or long scripts, read `docs/AGENT_PERFORMANCE.md`, write a
JSON script like `examples/vaelari_performance.json`, then run
`ruby bin/agent_perform --script PATH --output NEW_VIDEO.mp4`. The agent sets
dialogue, emotions, and timed yaw/pitch/roll cues; measured voice durations
drive the lip-sync and motion timelines. Do not put jaw or mouth commands in
head cues because the generated audio owns lip-sync.
On this Windows checkout, `scripts/perform.ps1 -ScriptPath PATH -OutputPath
NEW_VIDEO.mp4` uses the bundled Ruby and FFmpeg when `ruby` is not on PATH.
