#!/bin/sh
# Launch upstream Studio with this repository's native project definitions.
set -eu
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tools_dir=${ZEDFLOW_ADK_TOOLS:-"$HOME/.local/share/zedflow-adk-tools"}
studio_bin=${ADK_STUDIO_BIN:-"$tools_dir/bin/adk-studio"}
if [ ! -x "$studio_bin" ]; then
    studio_bin=$(command -v adk-studio || true)
fi
if [ -z "$studio_bin" ]; then
    echo 'Install ADK Studio 1.0.1 first; see docs/studio.md.' >&2
    exit 1
fi

# Optional user-local OpenSSL development files on the DGX (no system install).
ssl_dir="$tools_dir/openssl/usr"
if [ -f "$ssl_dir/lib/aarch64-linux-gnu/libssl.a" ]; then
    export OPENSSL_LIB_DIR="$ssl_dir/lib/aarch64-linux-gnu"
    export OPENSSL_INCLUDE_DIR="$ssl_dir/include"
    export OPENSSL_STATIC=1
    export CPATH="$ssl_dir/include/aarch64-linux-gnu${CPATH:+:$CPATH}"
fi
export TMPDIR=${ZEDFLOW_STUDIO_TMPDIR:-/tmp/zedflow-studio}
export CARGO_PROFILE_DEV_DEBUG=${CARGO_PROFILE_DEV_DEBUG:-0}
# Studio 1.0.1 requires a provider setting even for action-only workflows.
# This local-mode setting makes no request in our deterministic fixture project.
export OLLAMA_HOST=${OLLAMA_HOST:-http://127.0.0.1:11434}
mkdir -p "$TMPDIR" "$repo_dir/.adk-studio/projects"
cd "$repo_dir"
exec "$studio_bin" --dir "$repo_dir/.adk-studio/projects" "$@"
