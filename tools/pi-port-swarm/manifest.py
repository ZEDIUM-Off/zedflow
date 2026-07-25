#!/usr/bin/env python3
"""Deterministic closure checks for the frozen Pi source/test manifests."""
from __future__ import annotations

import argparse
import json
import subprocess
from collections import Counter
from pathlib import Path, PurePosixPath
from typing import Any

PACKAGES = ("ai", "agent", "tui", "coding-agent", "orchestrator")
DISPOSITIONS = {"consolidated", "type-only", "platform-specific", "live-capability", "dependency-arbitration"}


def files(root: Path, package: str) -> dict[str, str]:
    base = root / "references/pi/packages" / package
    result: dict[str, str] = {}
    for kind in ("src", "test"):
        directory = base / kind
        if directory.is_dir():
            for path in directory.rglob("*"):
                if path.is_file() and path.name.endswith((".ts", ".tsx", ".d.ts")):
                    result[path.relative_to(base).as_posix()] = kind
    for path in base.glob("*.d.ts"):
        result[path.relative_to(base).as_posix()] = "src"
    return result


def valid_target(target: str) -> bool:
    path = PurePosixPath(target)
    return bool(target) and not path.is_absolute() and ".." not in path.parts and path.parts[:1] == ("crates",)


def target_exists(root: Path, target: str, revision: str | None) -> bool:
    if not valid_target(target):
        return False
    if revision:
        return subprocess.run(["git", "cat-file", "-e", f"{revision}:{target}"], cwd=root, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    return (root / target).is_file()


def tsv(path: Path) -> list[list[str]]:
    if not path.is_file():
        return []
    return [line.split("\t") for line in path.read_text(encoding="utf-8").splitlines() if line]


def report(root: Path, package: str | None = None, revision: str | None = None) -> dict[str, Any]:
    selected = (package.removeprefix("zedflow-"),) if package else PACKAGES
    errors: list[str] = []
    if any(value not in PACKAGES for value in selected):
        errors.append(f"invalid package: {package}")
    manifest_dir = root / ".agents/port-manifests"
    dispositions: dict[str, list[dict[str, str]]] = {value: [] for value in selected if value in PACKAGES}
    for number, row in enumerate(tsv(manifest_dir / "exceptions.tsv"), 1):
        if len(row) != 5:
            errors.append(f"exceptions.tsv:{number}: expected package,source,disposition,target,evidence")
            continue
        row_package, source, disposition, target, evidence = row
        if row_package not in PACKAGES or disposition not in DISPOSITIONS or not source or not evidence:
            errors.append(f"exceptions.tsv:{number}: invalid disposition row")
            continue
        if row_package in dispositions:
            dispositions[row_package].append({"source": source, "disposition": disposition, "target": target, "evidence": evidence})
    packages: dict[str, Any] = {}
    for value in selected:
        if value not in PACKAGES:
            continue
        inventory = files(root, value)
        mappings: list[tuple[str, str, str]] = []
        for kind in ("src", "tests"):
            path = manifest_dir / f"{value}-{kind}.tsv"
            for number, row in enumerate(tsv(path), 1):
                if len(row) != 2 or not all(row):
                    errors.append(f"{path.name}:{number}: expected source and target")
                    continue
                source, target = row
                expected_kind = "test" if kind == "tests" else "src"
                if inventory.get(source) != expected_kind:
                    errors.append(f"{path.name}:{number}: source is not a frozen {expected_kind} TypeScript file: {source}")
                mappings.append((source, target, f"{path.name}:{number}"))
        sources = Counter(source for source, _, _ in mappings)
        duplicate_sources = sorted(source for source, count in sources.items() if count > 1)
        for source in duplicate_sources:
            errors.append(f"{value}: duplicate mapping for {source}")
        exception_sources = Counter(row["source"] for row in dispositions[value])
        mapping_targets = {source: target for source, target, _ in mappings}
        for source, count in sorted(exception_sources.items()):
            if count > 1:
                errors.append(f"{value}: duplicate disposition for {source}")
            if source not in inventory:
                errors.append(f"{value}: disposition source is not frozen: {source}")
            rows = [row for row in dispositions[value] if row["source"] == source]
            if source in sources and any(row["disposition"] not in {"consolidated", "platform-specific", "live-capability"} or row["target"] != mapping_targets[source] for row in rows):
                errors.append(f"{value}: mapping/disposition target mismatch: {source}")
        mapped = set(sources) | set(exception_sources)
        unlisted = sorted(set(inventory) - mapped)
        for source in unlisted:
            errors.append(f"{value}: unlisted frozen file: {source}")
        missing_targets = sorted(target for _, target, _ in mappings if not target_exists(root, target, revision))
        for row in dispositions[value]:
            disposition, target = row["disposition"], row["target"]
            if disposition == "type-only":
                if target:
                    errors.append(f"{value}: type-only disposition must not have a target: {row['source']}")
            elif disposition != "dependency-arbitration" and not target_exists(root, target, revision):
                missing_targets.append(target or f"<empty:{row['source']}>")
            if target and not valid_target(target):
                errors.append(f"{value}: disposition target escapes crates/: {target}")
        for target in sorted(set(missing_targets)):
            errors.append(f"{value}: missing target: {target}")
        target_sources: dict[str, list[str]] = {}
        for source, target, _ in mappings:
            target_sources.setdefault(target, []).append(source)
        consolidated_rows = [row for row in dispositions[value] if row["disposition"] == "consolidated"]
        consolidated_targets = Counter(row["target"] for row in consolidated_rows)
        for row in consolidated_rows:
            if not row["target"] or consolidated_targets[row["target"]] < 2:
                errors.append(f"{value}: consolidated disposition needs at least two sources for target: {row['source']}")
        duplicate_targets = {target: sorted(source_list) for target, source_list in target_sources.items() if len(source_list) > 1}
        consolidated_pairs = {(row["source"], row["target"]) for row in consolidated_rows}
        for target, source_list in sorted(duplicate_targets.items()):
            if any((source, target) not in consolidated_pairs for source in source_list):
                errors.append(f"{value}: duplicate exact target requires matching consolidated dispositions: {target}")
        arbitration = sorted(row["source"] for row in dispositions[value] if row["disposition"] == "dependency-arbitration")
        packages[f"zedflow-{value}"] = {
            "inventory": len(inventory),
            "mapped": len(mappings),
            "dispositions": {kind: sum(row["disposition"] == kind for row in dispositions[value]) for kind in sorted(DISPOSITIONS)},
            "exceptions": sorted(dispositions[value], key=lambda row: (row["source"], row["disposition"], row["target"])),
            "unlisted": unlisted,
            "missing_targets": sorted(set(missing_targets)),
            "duplicate_sources": duplicate_sources,
            "duplicate_targets": duplicate_targets,
            "dependency_arbitration": arbitration,
            "closed": not (unlisted or missing_targets or duplicate_sources or arbitration),
        }
    return {
        "status": "valid" if not errors and all(item["closed"] for item in packages.values()) else "blocked",
        "label": "deterministic frozen Pi manifest closure; target presence is mechanical, not fidelity completion",
        "revision": revision or "worktree",
        "packages": packages,
        "errors": sorted(errors),
        "dependency_arbitration": sorted(f"{name}:{source}" for name, item in packages.items() for source in item["dependency_arbitration"]),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("status", "check"))
    parser.add_argument("--package")
    parser.add_argument("--source", default=".")
    parser.add_argument("--revision")
    args = parser.parse_args()
    value = report(Path(args.source).resolve(), args.package, args.revision)
    print(json.dumps(value, sort_keys=True))
    return 0 if args.command == "status" or value["status"] == "valid" else 1


if __name__ == "__main__":
    raise SystemExit(main())
