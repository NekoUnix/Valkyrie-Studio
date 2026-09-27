# Vendored Purism Core

Upstream: https://github.com/SakuraMotion/PurismCore
Revision: `1069334965522df5d0b791e97f01e26b19c45456`
Version: 1.1.0; compatibility ABI: v6 (6.0.1).
License: MIT; see [LICENSE](LICENSE).

`PurismCoreBundle.h` is the amalgamation of the pinned upstream revision generated with upstream `scripts/bundle.sh` (UTF-8, LF line endings). SHA-256: `68e57128c18a489d01fb66497738abfeae448b266c302d25cef118012a264603`.

Only the library is vendored. Upstream test data, sample artwork, SDK binaries, and other sample dependencies are excluded. Cargo compiles it into Valkyrie Studio; no download or official Cubism SDK is used. Updates require review of upstream source and license changes, regeneration of the bundle and digest, and runtime/packaging checks. Keep this notice and the full MIT license with distributions.
