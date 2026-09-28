# Valkyrie Studio 0.3.0 alpha 4

Agent-controlled Vaelari now keeps moving between commands: gentle head and body sway, gaze motion, and blinking continue while the agent streams head tracking or the avatar speaks. The Inputs tab has a Motion energy slider and nod, shake, tilt, and lean preview buttons. The local agent API offers `puppet` (energy 0–2) and `gesture` commands so an AI director can adjust the same motion. Explicit tracking, audio-driven mouth movement, and direct model-parameter overrides retain priority.

Narrated performance scripts default to more expressive, line-varied head movement and motion energy 1.35. They accept `motion_energy` for the whole recording and an optional `gesture` on each line. The runner restores the previous live energy setting after a successful recording. The example script and user-facing guides show how to use these controls.

The six portable archives contain no model assets, API keys, official Cubism SDK, or FFmpeg executable. On Windows, 45 library tests and the performance-runner test passed, and the external local model diagnostic confirmed that agent motion reaches head, body, and eye parameters. Live visual review of this new motion and long narration capture are still outstanding; cross-platform CI checks builds and tests, not physical camera or GPU behavior on every platform.
