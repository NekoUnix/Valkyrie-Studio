#!/usr/bin/env bash
set -euo pipefail

label="${1:?Pass a platform label}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
stage="${root}/target/package/valkyrie-studio-${label}"
mkdir -p "${stage}/licenses" "${root}/dist"
cp "${root}/target/release/valkyrie-studio" "${stage}/"
cp "${root}/target/release/valkyrie-agent" "${stage}/"
cp "${root}/LICENSE" "${stage}/LICENSE"
cp "${root}/third_party/purism_core/LICENSE" "${stage}/licenses/PurismCore-LICENSE.txt"
cp "${root}/README.md" "${stage}/README.md"
tar -C "${root}/target/package" -czf "${root}/dist/valkyrie-studio-${label}.tar.gz" "valkyrie-studio-${label}"
