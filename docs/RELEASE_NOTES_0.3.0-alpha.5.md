# Valkyrie Studio 0.3.0 alpha 5

The Voice tab now walks through choosing a voice, directing its expression, and making a long video. Its expression strength slider runs from 0 to 5, Eleven v3 has a one-click selector, and a complete five-line performance script can be copied directly from the app.

Performance scripts can choose `eleven_v3` once for the whole video and give each line an `emotion` and `expression_intensity`. Valkyrie sends a matching Eleven v3 audio tag, applies the facial expression when the measured voice line starts, and adds a fitting gesture unless the script supplies one. Optional head cues run during that same line. The runner still generates speech before capture, so narration generation does not introduce pauses into the recorded video. Live typed speech and the local `tts` API can use the same emotion cue when playback starts.

The six portable archives contain no model files or credentials. Windows unit tests, script validation, and opt-in diagnostics with the external local model passed. Eleven v3 network generation, extended video capture, and hands-on macOS/Linux GUI operation remain to be checked on those services and devices; audio tags guide voice delivery but do not guarantee an exact acting style.
