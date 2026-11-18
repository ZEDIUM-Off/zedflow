#!/usr/bin/env bash
# Keep downloaded packages and install metadata inside ignored artifacts.
set -euo pipefail
workspace=$(cd "$(dirname "$0")/.." && pwd)
install="$workspace/.artifacts/packages/zedflow-oop"
mkdir -p "$install" "$workspace/.artifacts/tmp"
export TMPDIR="$workspace/.artifacts/tmp"
cp "$workspace/package.json" "$workspace/pnpm-workspace.yaml" "$install/"
if test -f "$workspace/pnpm-lock.yaml"; then cp "$workspace/pnpm-lock.yaml" "$install/"; fi
packages=()
for manifest in "$workspace"/packages/*/package.json; do
  package=$(basename "$(dirname "$manifest")")
  packages+=("packages/$package")
  mkdir -p "$install/packages/$package"
  cp "$manifest" "$install/packages/$package/"
  for source in "$workspace/packages/$package/"*; do
    name=$(basename "$source")
    case "$name" in package.json|node_modules) continue ;; esac
    link="$install/packages/$package/$name"
    if test -L "$link"; then test "$(readlink "$link")" = "$source";
    elif test -e "$link"; then echo "Refusing to replace $link" >&2; exit 1;
    else ln -s "$source" "$link"; fi
  done
done
if test "${1:-}" = --update-lockfile || ! test -f "$workspace/pnpm-lock.yaml"; then mode=--no-frozen-lockfile; else mode=--frozen-lockfile; fi
pnpm --dir "$install" --store-dir "$workspace/.artifacts/pnpm-store" --config.cache-dir="$workspace/.artifacts/pnpm-cache" install "$mode" --ignore-scripts
cp "$install/pnpm-lock.yaml" "$workspace/pnpm-lock.yaml"
for relative in . "${packages[@]}"; do
  source="$install/$relative/node_modules"
  destination="$workspace/$relative/node_modules"
  mkdir -p "$source"
  if test -L "$destination"; then test "$(readlink "$destination")" = "$source";
  elif test -e "$destination"; then echo "Refusing to replace $destination" >&2; exit 1;
  else ln -s "$source" "$destination"; fi
done
