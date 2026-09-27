# Agent connection to Valkyrie Studio

When asked to control the on-screen model, read `docs/CONNECT_AGENT.md` and `docs/API.md`. Keep Valkyrie Studio running with a local `.model3.json` loaded. Run `valkyrie-agent --connect` (or `valkyrie-agent.exe --connect` on Windows), verify READY, then send a visible command such as `valkyrie-agent '{"op":"emotion","name":"joy","duration":3}'`. On-screen persona: **Vaelari**; Odette is only the filename of the local test asset.

For smooth motion, reuse one authenticated JSON-lines TCP socket and send tracking around 20–30 Hz. The CLI reads the random token from the per-user data folder; do not reveal the token or provider keys. `schema` returns model-specific parameter IDs. Tell the user when a demo is scripted rather than a live language-model response.

For a narrated video, read `docs/AGENT_PERFORMANCE.md`, edit `examples/vaelari_performance.json`, and run `valkyrie-perform --script PATH --output NEW_VIDEO.webm`. It measures voice lines first and aligns head cues and lip-sync across chapters. Keep model files, still images of the model, and credentials out of Git. Only the existing cleared walkthrough video may be published.
