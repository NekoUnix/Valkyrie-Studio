# Third-party components and model rights

Valkyrie Studio's own source is [MIT-licensed](LICENSE). Its active native build uses [Purism Core](https://github.com/SakuraMotion/PurismCore), a separately developed `.moc3` runtime also released under MIT. The model loader, physics approximation, and OpenGL renderer in `native/` are this project's own code. The build does not download, compile, link, or package the official Live2D Cubism SDK or Cubism Framework. Purism is pinned to commit `1069334965522df5d0b791e97f01e26b19c45456`; its full MIT notice is in [licenses/PurismCore-LICENSE.txt](licenses/PurismCore-LICENSE.txt) and must accompany distributed copies.

The renderer change does not grant rights to any `.moc3`, texture, expression, or other model asset. The supplied Vaelari test model stays in its VTube Studio folder and is excluded from this repository. Permission to publish a video of that model does not by itself authorize distributing its files. Review the model creator's terms before packaging a model or using it commercially. Trademark, patent, and proprietary-format questions are distinct from Purism's MIT copyright license; this document does not give legal clearance for every use.

Downloaded dependencies live in ignored `vendor/`, `.tools/`, and `build/_deps/`. Preserve their upstream notices in any binary package. The components used or optionally installed are:

| Component | Version | License / note |
|---|---|---|
| Purism Core | Pinned commit above | MIT; full notice in `licenses/PurismCore-LICENSE.txt` |
| GLFW | 3.4 | zlib/libpng |
| Dear ImGui | 1.91.9b | MIT |
| nlohmann/json | 3.11.3 | MIT |
| miniaudio | 0.11.22 | Public domain or MIT-0; header notices |
| stb | Pinned `f0569113` | Public domain or MIT; header notices |
| glad loader | GLFW 3.4 bundled header | Generated loader and Khronos notices |
| Ruby and locked gems | `Gemfile.lock` | Per-gem licenses |
| FFmpeg | User-installed or local executable | License depends on that build; the development machine's BtbN build enables GPL components |
| MediaPipe/OpenCV | Optional webcam helper | Check installed distribution and model-asset terms |

The [existing walkthrough](media/Valkyrie-Studio-Walkthrough-Public.mp4) was recorded with the earlier official-SDK build. It is retained as a feature demonstration under the user's footage permission, not evidence of Purism rendering or a license for the model assets. A promotional still image is excluded because only video usage was confirmed. The previous local SDK download and build artifacts are ignored; they are not included in source or intended binary packages.
