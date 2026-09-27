#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
cmake -S native -B build -DCMAKE_BUILD_TYPE=Release "$@"
cmake --build build --config Release --target live2d_native_bridge --parallel
bundle install
printf '\nBuilt. Launch with: bundle exec ruby bin/studio --model /path/to/model.model3.json\n'
