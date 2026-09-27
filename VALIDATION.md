# Validation for the Rust alpha

## Tests and platform builds

GitHub Actions builds and runs Cargo tests on Windows x64, macOS Apple Silicon, macOS Intel, Ubuntu x64, Fedora x64, and Arch x64. All six jobs passed on the initial Rust PR commit. The final release-tag workflow must pass again for the final commit; [check its run](https://github.com/NekoUnix/Valkyrie-Studio/actions/workflows/rust-builds.yml). Cross-platform CI confirms compilation and unit behavior, not target-device GPU, camera, audio, or FFmpeg operation. The development PC's Windows release build is the only one exercised with the supplied local model.

## Windows model and export checks

The local Vaelari/Odette `.model3.json` opened with 208 parameters and 17 texture atlases. The model assets stayed in the Steam VTube Studio folder and are not part of Git. The Rust release build showed about 85–90 preview FPS at rest on this workstation with the face close-up. Debug builds were much slower and are not performance evidence. A real saved ElevenLabs credential fetched 23 account voices and generated a 2.93-second speech clip; playback changed the model mouth while tracking ran. The user-provided key itself is not in the repo.

A 1080×1920 two-line VP9/Opus recording lasted 5.9 seconds, retained alpha metadata, and was visually checked at early and late frames for mouth and head motion. A four-line performance split into two chapters, joined to an 11.8-second 1080×1920 VP9/Opus WebM with alpha metadata. The second chapter reported 75 captured frames, 14 repeated frames and 14 drops at 30 FPS. These are short integration checks, not a long-run throughput guarantee. One first attempt stalled in FFmpeg's capture stage and was stopped; a diagnostic rerun completed, so long-session reliability remains an open risk.

The independent spring physics loads the local model and renders without process errors; it is not verified as a pixel or motion match for Cubism Framework physics. Phone hardware, webcam hardware, macOS/Linux desktop operation, the other export codecs on target devices, and multi-hour narration still need tests. The public [walkthrough](media/Valkyrie-Studio-Walkthrough-Public.mp4) used an earlier renderer and should not be used as fidelity evidence for this Rust build.

For alpha 3, the A.R.I.A-derived solver passed 41 library tests, including fixed-step behavior at several frame rates, group isolation, energy bounds, pause recovery, and settings validation. The local model diagnostic loaded 62 physics groups and 208 parameters; a direct drive of its fast-tail input moved the output and the final one-second trace had no measurable oscillation at the calmer tail default. The optimized Windows binary compiled. The new Physics panel, native model picker, horizontal canvas control, and icon compile; this pass has not yet measured their live GUI behavior or horizontal video export.

The local GNU/LLVM Windows toolchain compiled the optimized Studio and Agent binaries, but its linker repeatedly received `Permission denied` while writing the separate `valkyrie-perform` executable. The source and unit tests for that binary passed. A complete alpha 3 portable archive therefore awaits the Windows CI build; the local package attempt did not complete.

The Windows portable archive was extracted to a separate folder and launched with the external local model. Its packaged `valkyrie-agent.exe --connect` reported READY. An archive entry check found the three binaries, docs, webcam helper, walkthrough, and Purism MIT notice, and found no model manifest, `.moc3`, session token, or credentials.
