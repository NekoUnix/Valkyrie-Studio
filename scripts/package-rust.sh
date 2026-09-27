#!/usr/bin/env bash
set -euo pipefail

label="${1:?Pass a platform label}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
stage="${root}/target/package/valkyrie-studio-${label}"
mkdir -p "${stage}/licenses" "${root}/dist"
cp "${root}/target/release/valkyrie-studio" "${stage}/"
cp "${root}/target/release/valkyrie-agent" "${stage}/"
cp "${root}/target/release/valkyrie-perform" "${stage}/"
cp "${root}/LICENSE" "${stage}/LICENSE"
cp "${root}/third_party/purism_core/LICENSE" "${stage}/licenses/PurismCore-LICENSE.txt"
cp "${root}/README.md" "${stage}/README.md"
cp "${root}/THIRD_PARTY.md" "${stage}/THIRD_PARTY.md"
cp -R "${root}/docs" "${root}/examples" "${root}/media" "${root}/helpers" "${stage}/"
mkdir -p "${stage}/scripts"
cp "${root}/scripts/webcam.ps1" "${stage}/scripts/"
tar -C "${root}/target/package" -czf "${root}/dist/valkyrie-studio-${label}.tar.gz" "valkyrie-studio-${label}"
