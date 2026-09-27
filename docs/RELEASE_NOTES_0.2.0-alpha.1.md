# Valkyrie Studio v0.2.0-alpha.1

This source-only alpha moves model evaluation to the MIT-licensed Purism Core runtime. The native bridge no longer builds or links the official Live2D Cubism SDK or Framework. It adds an independent, bounded 60 Hz spring solver for `physics3.json` rigs and reduces redundant OpenGL state changes when drawing dense models.

The source archive contains **no model files, model textures, provider credentials, or compiled application binaries**. Bring a model you have permission to use. The application source is MIT-licensed; Purism and the other dependencies have their own notices in [Third-party components](../THIRD_PARTY.md). Purism's license does not grant rights to a model or establish blanket legal clearance for `.moc3` use.

Windows x64 was tested with four local rigs. The supplied Vaelari rig loaded 208 parameters, 1,174 drawables, 17 textures, and 62 physics settings. A 1080×1920, 30 FPS transparent recording kept its alpha channel and dropped no encoder-queue frames in a short five-second run. A matched 300-frame renderer run was about 6% faster after the OpenGL change, with pixel-identical output. See [Validation](../VALIDATION.md) for methods and limits.

Build from source using [the platform guide](BUILD.md). Linux and macOS build paths are provided but have not been tested on target machines, so this release does not advertise native installers. The linked 81-second Vaelari walkthrough shows the features on the earlier renderer and is not a Purism rendering test.

Known limits: the independent physics solver can move differently from the rig author's original VTube Studio setup; offscreen compositing and extended blend modes are rejected; physical phone/webcam tracking, long recording sessions, and Linux/macOS runtime builds remain unverified. Start a dense rig at 30 FPS and lower canvas size if unique-frame capture falls behind.
