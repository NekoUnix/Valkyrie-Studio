# Valkyrie Studio 0.3.0 alpha 3

This alpha brings A.R.I.A's MIT-licensed particle physics into the Rust studio. The new Physics tab exposes Bouncy, Natural, and Authored motion, overall controls, searchable per-group tuning, reset actions, and settings saved for each model. A matching VTube Studio sidecar supplies supported strength, wind, and group multipliers; tail-named groups start with gentler inertia to reduce persistent oscillation. The solver runs independently of Purism Core, which still deforms the model mesh.

The studio now offers 16:9 and 9:16 canvas buttons, a native model browser, recent models, and startup reopening of the last available model. Voice explains Speak and animate in the app. The window and package include a new chibi phone icon. The desktop UI follows system DPI and includes social preview controls from the preceding UI update.

The bundled app and physics code are MIT licensed. No model assets, API keys, official Cubism SDK, or FFmpeg executable are included. The authorized walkthrough video is footage from the earlier renderer; it is not a fidelity comparison for this alpha.

Windows tests and the local model tail-drive diagnostic pass. CI builds all six platform archives; hands-on GUI and capture validation on macOS and Linux remains outstanding.
