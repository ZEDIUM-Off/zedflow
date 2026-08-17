#!/usr/bin/env python3
"""Generate and validate the frozen Pi↔Zedflow fidelity registry."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[2]
PI = ROOT / "references/pi"
REGISTRY = ROOT / "docs/porting/pi-fidelity-registry"
PI_REVISION = "2b00dade7cec918aefb025c8b7a4fa304a30acdd"
BASELINE_REVISION = "9564b26e2afd66d1c28258487c6bc290bc3d7c6f"
PACKAGES = ("ai", "agent", "tui", "coding-agent", "orchestrator")

HEADERS = {
    "behaviors.tsv": ("id", "package", "surface", "capability", "parent", "description"),
    "links.tsv": ("behavior_id", "relation", "target", "revision", "result"),
    "dependencies.tsv": ("behavior_id", "kind", "requirement", "unlock_state"),
    "dispositions.tsv": ("id", "scope_kind", "scope_id", "replaced_requirement", "issue_url", "approver", "status"),
}
RELATIONS = {
    "pi_source", "rust_implementation", "differential_case", "unit_test",
    "human_journey", "persistence_check", "review", "run_evidence",
}
DEPENDENCY_KINDS = {"requires_behavior", "requires_fixture", "requires_environment"}
RESULTS = {"source", "present", "red", "green", "approved", "not_applicable"}
DISPOSITION_STATUSES = {"pending", "approved", "rejected", "superseded"}
TEST_CALL = re.compile(r"(?<![\w.])(?:it|test)(?:\.(?:skip|only|todo|concurrent|each))*\s*\(")
TITLE = re.compile(r"^[\s\n]*(?:`([^`]+)`|'([^']+)'|\"([^\"]+)\")")
HEADING = re.compile(r"^(#{2,4})\s+(.+?)\s*$", re.MULTILINE)


def git(cwd: Path, *args: str, check: bool = True) -> str:
    completed = subprocess.run(
        ["git", "-C", str(cwd), *args], text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if check and completed.returncode:
        raise RuntimeError(completed.stderr.strip() or "git command failed")
    return completed.stdout


def tree_paths(cwd: Path, revision: str, *prefixes: str) -> list[str]:
    return sorted(filter(None, git(cwd, "ls-tree", "-r", "--name-only", revision, "--", *prefixes).splitlines()))


def pi_text(path: str) -> str:
    return git(PI, "show", f"{PI_REVISION}:{path}")


def baseline_text(path: str) -> str:
    return git(ROOT, "show", f"{BASELINE_REVISION}:{path}")


def git_object_exists(cwd: Path, object_name: str) -> bool:
    return subprocess.run(
        ["git", "-C", str(cwd), "cat-file", "-e", object_name],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    ).returncode == 0


def clean(value: str) -> str:
    return " ".join(value.replace("\t", " ").replace("\r", " ").replace("\n", " ").split())


def slug(value: str) -> str:
    value = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return value[:48] or "contract"


def behavior_id(package: str, surface: str, key: str, label: str) -> str:
    digest = hashlib.sha256(f"{package}\0{surface}\0{key}".encode()).hexdigest()[:10]
    return f"pi-{package}-{surface}-{slug(label)}-{digest}"


def package_for(path: str) -> str:
    match = re.match(r"packages/([^/]+)/", path)
    return match.group(1) if match and match.group(1) in PACKAGES else "coding-agent"


def capability(path: str) -> str:
    parts = PurePosixPath(path).parts
    if len(parts) >= 4 and parts[2] in {"src", "test", "docs"}:
        return parts[3].rsplit(".", 1)[0]
    return PurePosixPath(path).stem


def authoritative_paths() -> tuple[set[str], set[str], set[str]]:
    package_roots = tuple(f"packages/{package}" for package in PACKAGES)
    paths = tree_paths(PI, PI_REVISION, *package_roots)
    sources = {
        path for path in paths
        if "/src/" in path or re.fullmatch(r"packages/[^/]+/[^/]+\.d\.ts", path)
    }
    tests = {path for path in paths if "/test/" in path}
    docs = {
        path for path in paths
        if path.endswith(".md") and (
            "/docs/" in path or re.fullmatch(r"packages/[^/]+/README\.md", path)
        )
    }
    docs.add("README.md")
    manifests = {"package.json", *(f"packages/{package}/package.json" for package in PACKAGES)}
    return sources | tests, docs, manifests


def manifest_mappings() -> dict[tuple[str, str], str]:
    result: dict[tuple[str, str], str] = {}
    for package in PACKAGES:
        for kind in ("src", "tests"):
            path = f".agents/port-manifests/{package}-{kind}.tsv"
            for line in baseline_text(path).splitlines():
                if line:
                    source, target = line.split("\t")
                    result[(package, source)] = target
    return result


def add_behavior(
    behaviors: list[tuple[str, ...]], links: list[tuple[str, ...]], dependencies: list[tuple[str, ...]],
    mappings: dict[tuple[str, str], str], *, package: str, surface: str, key: str,
    label: str, description: str, pi_path: str, source_kind: str,
) -> None:
    identifier = behavior_id(package, surface, key, label)
    behaviors.append((identifier, package, surface, capability(pi_path), "", clean(description)))
    links.append((identifier, "pi_source", f"references/pi/{pi_path}", PI_REVISION, "source"))

    relative = pi_path.removeprefix(f"packages/{package}/")
    mapped_target = mappings.get((package, relative))
    implementation_target = mapped_target if source_kind == "source" else None
    unit_target = mapped_target if source_kind == "test" else None
    links.append((
        identifier, "rust_implementation", implementation_target or f"planned://rust/{identifier}",
        BASELINE_REVISION if implementation_target else "", "present" if implementation_target else "red",
    ))
    links.append((
        identifier, "unit_test", unit_target or f"planned://unit-test/{identifier}",
        BASELINE_REVISION if unit_target else "", "red",
    ))
    links.extend((
        (identifier, "differential_case", f"planned://differential-case/{identifier}", "", "red"),
        (identifier, "human_journey", f"planned://human-journey/{package}/{surface}", "", "red"),
        (identifier, "persistence_check", f"planned://persistence-check/{identifier}", "", "red"),
        (identifier, "review", f"planned://review/{identifier}", "", "red"),
        (identifier, "run_evidence", f"planned://run-evidence/{identifier}", "", "red"),
    ))
    dependencies.append((identifier, "requires_fixture", f"planned://fixture/{identifier}", "green"))


def build_rows() -> dict[str, list[tuple[str, ...]]]:
    mappings = manifest_mappings()
    behaviors: list[tuple[str, ...]] = []
    links: list[tuple[str, ...]] = []
    dependencies: list[tuple[str, ...]] = []
    artifacts, docs, manifests = authoritative_paths()

    for path in sorted(artifacts):
        package = package_for(path)
        kind = "test" if "/test/" in path else "source"
        text = pi_text(path) if path.endswith((".ts", ".tsx", ".js", ".mjs")) else ""
        matches = list(TEST_CALL.finditer(text)) if kind == "test" else []
        if matches:
            for index, match in enumerate(matches, 1):
                tail = text[match.end():match.end() + 240]
                title_match = TITLE.match(tail)
                title = next((group for group in title_match.groups() if group), "dynamic declaration") if title_match else "dynamic declaration"
                line = text.count("\n", 0, match.start()) + 1
                add_behavior(
                    behaviors, links, dependencies, mappings,
                    package=package, surface="test-scenario", key=f"{path}:{index}:{title}",
                    label=title, description=f"{title} ({path}:{line})", pi_path=path, source_kind=kind,
                )
        else:
            label = PurePosixPath(path).name
            add_behavior(
                behaviors, links, dependencies, mappings,
                package=package, surface="test-artifact" if kind == "test" else "source-contract",
                key=path, label=label, description=f"Contract carried by {path}",
                pi_path=path, source_kind=kind,
            )

    for path in sorted(docs):
        package = package_for(path)
        text = pi_text(path)
        headings = list(HEADING.finditer(text))
        for index, match in enumerate(headings or [None], 1):
            title = match.group(2) if match else PurePosixPath(path).name
            add_behavior(
                behaviors, links, dependencies, mappings,
                package=package, surface="documented-contract", key=f"{path}:{index}:{title}",
                label=title, description=f"Documented contract: {title} ({path})",
                pi_path=path, source_kind="doc",
            )

    for path in sorted(manifests):
        package = package_for(path)
        data = json.loads(pi_text(path))
        entries: list[tuple[str, str]] = [("manifest", "package manifest")]
        exports = data.get("exports", {})
        if isinstance(exports, dict):
            entries.extend((f"export:{name}", f"public package export {name}") for name in exports)
        binary = data.get("bin", {})
        if isinstance(binary, str):
            entries.append(("bin", "package executable"))
        elif isinstance(binary, dict):
            entries.extend((f"bin:{name}", f"package executable {name}") for name in binary)
        for key, label in entries:
            add_behavior(
                behaviors, links, dependencies, mappings,
                package=package, surface="package-contract", key=f"{path}:{key}",
                label=label, description=f"{label} ({path})", pi_path=path, source_kind="manifest",
            )

    # Existing whole-boundary differential fixtures are exact red seeds, not passing evidence.
    fixture_groups = (
        ("tui", "existing-component-fixture", "tools/tui-parity/fixtures", BASELINE_REVISION),
        ("coding-agent", "existing-cli-fixture", "tools/tui-fidelity/fixtures", BASELINE_REVISION),
    )
    local_paths = set(tree_paths(ROOT, BASELINE_REVISION, *(group[2] for group in fixture_groups)))
    for package, surface, directory, revision in fixture_groups:
        for path in sorted(p for p in local_paths if p.startswith(directory + "/") and p.endswith(".json") and not p.endswith("/schema.json")):
            identifier = behavior_id(package, surface, path, PurePosixPath(path).stem)
            pi_path = "packages/tui/src/tui.ts" if package == "tui" else "packages/coding-agent/src/modes/interactive/interactive-mode.ts"
            behaviors.append((identifier, package, surface, capability(pi_path), "", f"Existing red differential fixture {path}"))
            links.extend((
                (identifier, "pi_source", f"references/pi/{pi_path}", PI_REVISION, "source"),
                (identifier, "rust_implementation", f"planned://rust/{identifier}", "", "red"),
                (identifier, "unit_test", f"planned://unit-test/{identifier}", "", "red"),
                (identifier, "differential_case", path, revision, "red"),
                (identifier, "human_journey", f"planned://human-journey/{package}/{surface}", "", "red"),
                (identifier, "persistence_check", f"planned://persistence-check/{identifier}", "", "red"),
                (identifier, "review", f"planned://review/{identifier}", "", "red"),
                (identifier, "run_evidence", f"planned://run-evidence/{identifier}", "", "red"),
            ))
            dependencies.append((identifier, "requires_fixture", path, "green"))

    behaviors.sort()
    links.sort()
    dependencies.sort()
    return {
        "behaviors.tsv": behaviors,
        "links.tsv": links,
        "dependencies.tsv": dependencies,
        "dispositions.tsv": [],
    }


def write_rows(rows: dict[str, list[tuple[str, ...]]]) -> None:
    REGISTRY.mkdir(parents=True, exist_ok=True)
    for name, values in rows.items():
        with (REGISTRY / name).open("w", encoding="utf-8", newline="") as file:
            writer = csv.writer(file, delimiter="\t", lineterminator="\n")
            writer.writerow(HEADERS[name])
            writer.writerows(values)


def read_rows(name: str) -> list[tuple[str, ...]]:
    path = REGISTRY / name
    if not path.is_file():
        raise ValueError(f"missing {path.relative_to(ROOT)}")
    with path.open(encoding="utf-8", newline="") as file:
        raw = list(csv.reader(file, delimiter="\t"))
    if not raw or tuple(raw[0]) != HEADERS[name]:
        raise ValueError(f"{name}: expected header {' '.join(HEADERS[name])}")
    width = len(HEADERS[name])
    for number, row in enumerate(raw[1:], 2):
        if len(row) != width or not all(value.strip() == value for value in row):
            raise ValueError(f"{name}:{number}: malformed row")
    return [tuple(row) for row in raw[1:]]


def validate() -> dict[str, int]:
    if git(ROOT, "rev-parse", f"{BASELINE_REVISION}:references/pi").strip() != PI_REVISION:
        raise ValueError("baseline does not contain the frozen Pi gitlink")
    if subprocess.run(["git", "-C", str(ROOT), "merge-base", "--is-ancestor", BASELINE_REVISION, "HEAD"]).returncode:
        raise ValueError("current HEAD does not descend from the recorded baseline")
    if git(PI, "rev-parse", "HEAD").strip() != PI_REVISION:
        raise ValueError("references/pi HEAD differs from the frozen gitlink")

    actual = {name: read_rows(name) for name in HEADERS}
    expected = build_rows()
    for name in HEADERS:
        if actual[name] != expected[name]:
            actual_set, expected_set = set(actual[name]), set(expected[name])
            missing = sorted(expected_set - actual_set)[:3]
            extra = sorted(actual_set - expected_set)[:3]
            raise ValueError(f"{name}: stale registry (missing={missing!r}, extra={extra!r})")

    behaviors = actual["behaviors.tsv"]
    links = actual["links.tsv"]
    dependencies = actual["dependencies.tsv"]
    dispositions = actual["dispositions.tsv"]
    ids = [row[0] for row in behaviors]
    if len(ids) != len(set(ids)):
        raise ValueError("behaviors.tsv: duplicate id")
    known = set(ids)
    by_behavior: dict[str, set[str]] = defaultdict(set)
    for behavior, relation, target, revision, result in links:
        if behavior not in known or relation not in RELATIONS or result not in RESULTS or not target:
            raise ValueError(f"links.tsv: invalid link for {behavior}")
        by_behavior[behavior].add(relation)
        if relation == "pi_source":
            if revision != PI_REVISION or result != "source":
                raise ValueError(f"links.tsv: inconsistent Pi revision for {behavior}")
            path = target.removeprefix("references/pi/")
            if target == path or not git_object_exists(PI, f"{PI_REVISION}:{path}"):
                raise ValueError(f"links.tsv: missing frozen Pi target {target}")
        elif result in {"present", "green"} and revision:
            if not git_object_exists(ROOT, f"{revision}:{target}"):
                raise ValueError(f"links.tsv: missing repository target {target}@{revision}")
    if len(links) != len(set(links)):
        raise ValueError("links.tsv: duplicate link")
    if any(set(RELATIONS) - by_behavior[identifier] for identifier in known):
        raise ValueError("links.tsv: every behavior must name all required proof relations")

    graph: dict[str, set[str]] = defaultdict(set)
    for behavior, kind, requirement, unlock_state in dependencies:
        if behavior not in known or kind not in DEPENDENCY_KINDS or not requirement or not unlock_state:
            raise ValueError(f"dependencies.tsv: invalid dependency for {behavior}")
        if kind == "requires_behavior":
            if requirement not in known or requirement == behavior:
                raise ValueError(f"dependencies.tsv: invalid behavior edge for {behavior}")
            graph[behavior].add(requirement)
    visiting: set[str] = set()
    visited: set[str] = set()
    def visit(node: str) -> None:
        if node in visiting:
            raise ValueError("dependencies.tsv: cycle")
        if node not in visited:
            visiting.add(node)
            for dependency in graph[node]:
                visit(dependency)
            visiting.remove(node)
            visited.add(node)
    for identifier in known:
        visit(identifier)

    disposition_ids: set[str] = set()
    for identifier, scope_kind, scope_id, requirement, issue_url, approver, status in dispositions:
        if identifier in disposition_ids or scope_kind not in {"behavior", "relation", "source"} or status not in DISPOSITION_STATUSES:
            raise ValueError(f"dispositions.tsv: invalid disposition {identifier}")
        if not scope_id or not requirement or not issue_url.startswith("https://github.com/") or not approver:
            raise ValueError(f"dispositions.tsv: incomplete disposition {identifier}")
        disposition_ids.add(identifier)

    authoritative = set().union(*authoritative_paths())
    covered = {
        target.removeprefix("references/pi/")
        for _, relation, target, _, _ in links if relation == "pi_source"
    }
    if authoritative != covered:
        raise ValueError(f"bidirectional Pi coverage failed: missing={sorted(authoritative-covered)[:3]}, extra={sorted(covered-authoritative)[:3]}")

    red_cases = sum(relation == "differential_case" and result == "red" for _, relation, _, _, result in links)
    fixtures = sum(kind == "requires_fixture" for _, kind, _, _ in dependencies)
    if red_cases != len(behaviors) or fixtures != len(behaviors):
        raise ValueError("every inventoried behavior must have one red case and fixture")
    return {
        "behaviors": len(behaviors), "links": len(links), "dependencies": len(dependencies),
        "dispositions": len(dispositions), "pi_files": len(authoritative),
        "red_cases": red_cases, "red_fixtures": fixtures,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="regenerate the four canonical TSVs")
    args = parser.parse_args()
    try:
        if args.write:
            write_rows(build_rows())
        counts = validate()
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(f"invalid: {error}", file=sys.stderr)
        raise SystemExit(1)
    print("valid: " + ", ".join(f"{key}={value}" for key, value in counts.items()))


if __name__ == "__main__":
    main()
