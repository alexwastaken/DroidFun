#!/bin/bash
set -euo pipefail
repo_dir=$(cd "$(dirname "$0")/.." && pwd)
exec "$repo_dir/src-tauri/resources/picotool-macos-universal/build-helper.sh" "$@"
