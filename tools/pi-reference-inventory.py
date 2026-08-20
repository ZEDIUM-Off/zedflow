#!/usr/bin/env python3
"""Rebuild the normative Pi reference inventory from the pinned git tree."""

import argparse
import json
import subprocess
from pathlib import Path, PurePosixPath

PIN = "914cf1472e715297caa30db4b9535d534a9eb718"
ROOT = Path(__file__).resolve().parents[1]
PI = ROOT / "references/pi"
OUT = ROOT / "docs/porting/pi-v0.84.2-inventory"


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(PI), *args])


def tracked_files() -> list[tuple[str, str]]:
    rows = []
    for entry in git("ls-tree", "-r", "-z", PIN).split(b"\0"):
        if not entry:
            continue
        metadata, raw_path = entry.split(b"\t", 1)
        oid = metadata.split()[2].decode()
        rows.append((raw_path.decode(), oid))
    return rows


def package_manifests(files: list[tuple[str, str]]) -> list[tuple[str, dict]]:
    packages = []
    for path, _ in files:
        if not path.startswith("packages/") or not path.endswith("/package.json"):
            continue
        data = json.loads(git("show", f"{PIN}:{path}"))
        if not data.get("private", False):
            packages.append((str(PurePosixPath(path).parent), data))
    return sorted(packages)


def classify(relative: str) -> str:
    path = PurePosixPath(relative)
    parts = path.parts
    name = path.name.lower()
    if name in {"package.json", "package-lock.json", "npm-shrinkwrap.json"} or (
        name.startswith("tsconfig") and name.endswith(".json")
    ):
        return "manifest"
    if "test" in parts or "tests" in parts or ".test." in name or ".spec." in name:
        return "test"
    if "docs" in parts or name.startswith("readme") or path.suffix.lower() == ".md":
        return "doc"
    if "src" in parts or "scripts" in parts or "native" in parts or path.suffix.lower() in {
        ".c",
        ".cjs",
        ".h",
        ".js",
        ".mjs",
        ".sh",
        ".ts",
        ".tsx",
    }:
        return "source"
    return "asset"


def render() -> dict[Path, str]:
    files = tracked_files()
    packages = package_manifests(files)
    names = {data["name"] for _, data in packages}

    nodes = [
        {
            "name": data["name"],
            "path": path,
            "version": data["version"],
        }
        for path, data in packages
    ]
    edges = []
    for path, data in packages:
        for kind in ("dependencies", "optionalDependencies", "peerDependencies", "devDependencies"):
            for target, requirement in sorted(data.get(kind, {}).items()):
                if target in names:
                    edges.append(
                        {
                            "from": data["name"],
                            "kind": kind,
                            "requirement": requirement,
                            "to": target,
                        }
                    )
    graph = json.dumps(
        {"pi_revision": PIN, "nodes": nodes, "edges": sorted(edges, key=lambda row: tuple(row.values()))},
        indent=2,
        sort_keys=True,
    ) + "\n"

    inventory = ["package\tkind\tpath\tgit_blob"]
    roots = [(PurePosixPath(path), data["name"]) for path, data in packages]
    for path, oid in files:
        owner = next((name for root, name in roots if PurePosixPath(path).is_relative_to(root)), None)
        if owner is None:
            if "/" in path or (classify(path) != "manifest" and path != "README.md"):
                continue
            owner = "pi-monorepo"
        inventory.append(f"{owner}\t{classify(path)}\t{path}\t{oid}")

    readme = f"""# Pi v0.84.2 normative inventory

This directory records the published-package graph and the complete tracked-file inventory used by the Pi fidelity campaign.

- Pi revision: `v0.84.2@{PIN}`
- Scope: every non-private package under `references/pi/packages`, plus the root package/build manifests, lockfile, and README.
- `normative-files.tsv` includes every tracked file owned by that scope. `kind` is a navigation aid; it does not decide whether a file defines a behavior.
- `package-graph.json` records every internal dependency edge declared by a published package, including development edges.
- Git blob IDs are read from the pinned commit, never from the working tree. Historical diffs are not used to select files.

Rebuild or verify from the repository root:

```bash
python3 tools/pi-reference-inventory.py
python3 tools/pi-reference-inventory.py --check
```
"""
    return {
        OUT / "README.md": readme,
        OUT / "package-graph.json": graph,
        OUT / "normative-files.tsv": "\n".join(inventory) + "\n",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    revision = git("rev-parse", "HEAD").decode().strip()
    if revision != PIN:
        raise SystemExit(f"references/pi is {revision}; expected {PIN}")

    outputs = render()
    if args.check:
        stale = [str(path.relative_to(ROOT)) for path, content in outputs.items() if not path.exists() or path.read_text() != content]
        if stale:
            raise SystemExit("stale inventory: " + ", ".join(stale))
        return 0

    OUT.mkdir(parents=True, exist_ok=True)
    for path, content in outputs.items():
        path.write_text(content)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
