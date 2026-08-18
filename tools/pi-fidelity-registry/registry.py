#!/usr/bin/env python3
"""Validate the editable Pi↔Zedflow fidelity registry."""
from __future__ import annotations

import argparse
import csv
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PI = ROOT / "references/pi"
REGISTRY = ROOT / "docs/porting/pi-fidelity-registry"
PI_REVISION = "2b00dade7cec918aefb025c8b7a4fa304a30acdd"
BASELINE_REVISION = "e91b44be9c897aef63c84c34b4e14b387a8141a7"
PACKAGES = {"ai", "agent", "tui", "coding-agent", "orchestrator"}

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
ALLOWED_RESULTS = {
    "pi_source": {"source"},
    "rust_implementation": {"red", "present"},
    "differential_case": {"red", "green", "not_applicable"},
    "unit_test": {"red", "green", "not_applicable"},
    "human_journey": {"red", "approved", "not_applicable"},
    "persistence_check": {"red", "green", "not_applicable"},
    "review": {"red", "approved", "not_applicable"},
    "run_evidence": {"red", "green", "not_applicable"},
}
DISPOSITION_STATUSES = {"pending", "approved", "rejected", "superseded"}
PASS_RESULT = {
    "pi_source": "source",
    "rust_implementation": "present",
    "differential_case": "green",
    "unit_test": "green",
    "human_journey": "approved",
    "persistence_check": "green",
    "review": "approved",
    "run_evidence": "green",
}
SHA = re.compile(r"[0-9a-f]{40}")


def git(cwd: Path, *args: str, check: bool = True) -> str:
    completed = subprocess.run(
        ["git", "-C", str(cwd), *args], text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if check and completed.returncode:
        raise RuntimeError(completed.stderr.strip() or "git command failed")
    return completed.stdout


def tree_paths(cwd: Path, revision: str, *prefixes: str) -> set[str]:
    return set(filter(None, git(cwd, "ls-tree", "-r", "--name-only", revision, "--", *prefixes).splitlines()))


def git_object_exists(cwd: Path, object_name: str) -> bool:
    return subprocess.run(
        ["git", "-C", str(cwd), "cat-file", "-e", object_name],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    ).returncode == 0


def authoritative_paths() -> set[str]:
    roots = tuple(f"packages/{package}" for package in sorted(PACKAGES))
    paths = tree_paths(PI, PI_REVISION, *roots)
    artifacts = {
        path for path in paths
        if "/src/" in path or "/test/" in path
        or re.fullmatch(r"packages/[^/]+/[^/]+\.d\.ts", path)
    }
    docs = {
        path for path in paths
        if path.endswith(".md") and (
            "/docs/" in path or re.fullmatch(r"packages/[^/]+/README\.md", path)
        )
    }
    return artifacts | docs | {
        "README.md", "package.json", *(f"packages/{package}/package.json" for package in PACKAGES),
    }


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


def approved_dispositions(rows: list[tuple[str, ...]]) -> set[tuple[str, str, str]]:
    return {(scope_kind, scope_id, requirement) for _, scope_kind, scope_id, requirement, _, _, status in rows if status == "approved"}


def relation_passes(
    behavior: str,
    relation: str,
    rows: list[tuple[str, ...]],
    approved: set[tuple[str, str, str]],
) -> bool:
    if ("behavior", behavior, relation) in approved or ("relation", behavior, relation) in approved:
        return True
    expected = PASS_RESULT[relation]
    matching = [row for row in rows if row[1] == relation]
    return bool(matching) and all(row[4] == expected for row in matching)


def validate(require_complete: bool = False) -> tuple[dict[str, int], list[str]]:
    if git(ROOT, "rev-parse", f"{BASELINE_REVISION}:references/pi").strip() != PI_REVISION:
        raise ValueError("baseline does not contain the frozen Pi gitlink")
    if not git_object_exists(PI, PI_REVISION):
        raise ValueError("frozen Pi object is unavailable")

    actual = {name: read_rows(name) for name in HEADERS}
    behaviors = actual["behaviors.tsv"]
    links = actual["links.tsv"]
    dependencies = actual["dependencies.tsv"]
    dispositions = actual["dispositions.tsv"]

    ids = [row[0] for row in behaviors]
    if len(ids) != len(set(ids)):
        raise ValueError("behaviors.tsv: duplicate id")
    known = set(ids)
    for identifier, package, surface, capability, parent, description in behaviors:
        if not identifier or package not in PACKAGES or not surface or not capability or not description:
            raise ValueError(f"behaviors.tsv: incomplete behavior {identifier!r}")

    disposition_ids: set[str] = set()
    for identifier, scope_kind, scope_id, requirement, issue_url, approver, status in dispositions:
        if identifier in disposition_ids or scope_kind not in {"behavior", "relation", "source"} or status not in DISPOSITION_STATUSES:
            raise ValueError(f"dispositions.tsv: invalid disposition {identifier}")
        if not scope_id or not requirement or not issue_url.startswith("https://github.com/") or not approver:
            raise ValueError(f"dispositions.tsv: incomplete disposition {identifier}")
        if scope_kind in {"behavior", "relation"} and scope_id not in known:
            raise ValueError(f"dispositions.tsv: unknown behavior {scope_id}")
        if scope_kind in {"behavior", "relation"} and requirement not in RELATIONS:
            raise ValueError(f"dispositions.tsv: unknown replaced relation {requirement}")
        if scope_kind == "source" and requirement != "pi_source":
            raise ValueError(f"dispositions.tsv: source disposition must replace pi_source")
        disposition_ids.add(identifier)
    approved = approved_dispositions(dispositions)

    by_behavior: dict[str, list[tuple[str, ...]]] = defaultdict(list)
    seen_links: set[tuple[str, ...]] = set()
    for row in links:
        behavior, relation, target, revision, result = row
        if row in seen_links:
            raise ValueError(f"links.tsv: duplicate link {row}")
        seen_links.add(row)
        if behavior not in known or relation not in RELATIONS or result not in RESULTS or not target:
            raise ValueError(f"links.tsv: invalid link for {behavior}")
        if result not in ALLOWED_RESULTS[relation]:
            raise ValueError(f"links.tsv: invalid {result} result for {relation}")
        by_behavior[behavior].append(row)
        if relation == "pi_source":
            path = target.removeprefix("references/pi/")
            if target == path or revision != PI_REVISION or result != "source" or not git_object_exists(PI, f"{PI_REVISION}:{path}"):
                raise ValueError(f"links.tsv: invalid frozen Pi target {target}")
        elif result in {"present", "green", "approved"}:
            if target.startswith("planned://") or revision != BASELINE_REVISION or not git_object_exists(ROOT, f"{revision}:{target}"):
                raise ValueError(f"links.tsv: passing target must exist at {BASELINE_REVISION}: {target}")
        elif revision:
            if not SHA.fullmatch(revision) or target.startswith("planned://") or not git_object_exists(ROOT, f"{revision}:{target}"):
                raise ValueError(f"links.tsv: invalid immutable target {target}@{revision}")
        elif result == "not_applicable" and ("relation", behavior, relation) not in approved:
            raise ValueError(f"links.tsv: not_applicable requires an approved relation disposition for {behavior}:{relation}")

    for identifier in known:
        missing = RELATIONS - {row[1] for row in by_behavior[identifier]}
        missing -= {requirement for kind, scope, requirement in approved if kind in {"behavior", "relation"} and scope == identifier}
        if missing:
            raise ValueError(f"links.tsv: {identifier} lacks {sorted(missing)}")

    if len(dependencies) != len(set(dependencies)):
        raise ValueError("dependencies.tsv: duplicate dependency")
    graph: dict[str, set[str]] = defaultdict(set)
    dependency_rows: dict[str, list[tuple[str, ...]]] = defaultdict(list)
    for behavior, kind, requirement, unlock_state in dependencies:
        if behavior not in known or kind not in DEPENDENCY_KINDS or not requirement or unlock_state not in {"pending", "satisfied"}:
            raise ValueError(f"dependencies.tsv: invalid dependency for {behavior}")
        dependency_rows[behavior].append((behavior, kind, requirement, unlock_state))
        if kind == "requires_behavior":
            if requirement not in known or requirement == behavior:
                raise ValueError(f"dependencies.tsv: invalid behavior edge for {behavior}")
            graph[behavior].add(requirement)
        elif unlock_state == "satisfied" and (requirement.startswith("planned://") or not (ROOT / requirement).exists()):
            raise ValueError(f"dependencies.tsv: satisfied requirement does not exist: {requirement}")

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

    base_status: dict[str, str] = {}
    stages = ("inventoried", "red", "implemented", "differential_green", "reviewed", "human_validated")
    for identifier in known:
        passed = {relation: relation_passes(identifier, relation, by_behavior[identifier], approved) for relation in RELATIONS}
        status = "inventoried"
        if any(row[4] == "red" for row in by_behavior[identifier]):
            status = "red"
        if passed["rust_implementation"]:
            status = "implemented"
        if all(passed[relation] for relation in ("rust_implementation", "differential_case", "unit_test", "run_evidence")):
            status = "differential_green"
        if status == "differential_green" and passed["review"]:
            status = "reviewed"
        if status == "reviewed" and passed["human_journey"]:
            status = "human_validated"
        base_status[identifier] = status

    rank = {stage: index for index, stage in enumerate(stages)}
    for behavior, kind, requirement, unlock_state in dependencies:
        if kind == "requires_behavior" and unlock_state == "satisfied" and rank[base_status[requirement]] < rank["differential_green"]:
            raise ValueError(f"dependencies.tsv: {behavior} is satisfied by unready behavior {requirement}")

    statuses = dict(base_status)
    for identifier in known:
        passed = {relation: relation_passes(identifier, relation, by_behavior[identifier], approved) for relation in RELATIONS}
        deps_satisfied = all(
            state == "satisfied" and (kind != "requires_behavior" or rank[base_status[requirement]] >= rank["differential_green"])
            for _, kind, requirement, state in dependency_rows[identifier]
        )
        no_pending_disposition = not any(row[2] == identifier and row[6] == "pending" for row in dispositions)
        if all(passed.values()) and deps_satisfied and no_pending_disposition:
            statuses[identifier] = "complete"

    authoritative = authoritative_paths()
    invalid_source_dispositions = {
        scope.removeprefix("references/pi/")
        for _, scope_kind, scope, requirement, _, _, _ in dispositions
        if scope_kind == "source" and requirement == "pi_source"
    } - authoritative
    if invalid_source_dispositions:
        raise ValueError(f"dispositions.tsv: non-authoritative source {sorted(invalid_source_dispositions)[:3]}")
    covered = {
        target.removeprefix("references/pi/")
        for _, relation, target, _, _ in links if relation == "pi_source"
    }
    covered |= {
        scope.removeprefix("references/pi/")
        for kind, scope, requirement in approved if kind == "source" and requirement == "pi_source"
    }
    extra = covered - authoritative
    if extra:
        raise ValueError(f"Pi coverage contains non-authoritative paths: {sorted(extra)[:3]}")
    uncovered = sorted(authoritative - covered)

    counts = Counter(statuses.values())
    result = {
        "behaviors": len(behaviors), "links": len(links), "dependencies": len(dependencies),
        "dispositions": len(dispositions), "pi_anchors": len(authoritative),
        "covered_anchors": len(authoritative) - len(uncovered), "uncovered_anchors": len(uncovered),
        **{stage: counts[stage] for stage in (*stages, "complete")},
    }
    if require_complete and (uncovered or counts["complete"] != len(behaviors)):
        raise ValueError(f"registry is incomplete: uncovered_anchors={len(uncovered)}, incomplete_behaviors={len(behaviors) - counts['complete']}")
    return result, uncovered


def self_check() -> None:
    import tempfile

    rows = [("b", "differential_case", "fixture", BASELINE_REVISION, "green")]
    assert relation_passes("b", "differential_case", rows, set())
    assert not relation_passes("b", "unit_test", rows, set())
    assert relation_passes("b", "unit_test", rows, {("relation", "b", "unit_test")})

    global REGISTRY
    original = REGISTRY
    with tempfile.TemporaryDirectory() as directory:
        REGISTRY = Path(directory)
        sample = {
            "behaviors.tsv": [("b", "agent", "api", "events", "", "Emits one event")],
            "links.tsv": [
                ("b", "pi_source", "references/pi/README.md", PI_REVISION, "source"),
                ("b", "rust_implementation", "README.md", BASELINE_REVISION, "present"),
                ("b", "differential_case", "README.md", BASELINE_REVISION, "green"),
                ("b", "unit_test", "README.md", BASELINE_REVISION, "green"),
                ("b", "human_journey", "README.md", BASELINE_REVISION, "approved"),
                ("b", "persistence_check", "README.md", BASELINE_REVISION, "green"),
                ("b", "review", "README.md", BASELINE_REVISION, "approved"),
                ("b", "run_evidence", "README.md", BASELINE_REVISION, "green"),
            ],
            "dependencies.tsv": [],
            "dispositions.tsv": [],
        }
        for name, values in sample.items():
            with (REGISTRY / name).open("w", encoding="utf-8", newline="") as file:
                writer = csv.writer(file, delimiter="\t", lineterminator="\n")
                writer.writerow(HEADERS[name])
                writer.writerows(values)
        counts, uncovered = validate()
        assert counts["complete"] == 1 and len(uncovered) == 776
        sample["links.tsv"][2] = ("b", "differential_case", "planned://case", "", "green")
        with (REGISTRY / "links.tsv").open("w", encoding="utf-8", newline="") as file:
            writer = csv.writer(file, delimiter="\t", lineterminator="\n")
            writer.writerow(HEADERS["links.tsv"])
            writer.writerows(sample["links.tsv"])
        try:
            validate()
        except ValueError:
            pass
        else:
            raise AssertionError("planned green evidence was accepted")
    REGISTRY = original


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-complete", action="store_true", help="fail unless every behavior and Pi anchor is complete")
    parser.add_argument("--uncovered", action="store_true", help="print uncovered frozen Pi anchors")
    parser.add_argument("--self-check", action="store_true", help="run the validator's pure-logic checks")
    args = parser.parse_args()
    try:
        if args.self_check:
            self_check()
        counts, uncovered = validate(args.require_complete)
    except (OSError, RuntimeError, ValueError) as error:
        print(f"invalid: {error}", file=sys.stderr)
        raise SystemExit(1)
    print("valid: " + ", ".join(f"{key}={value}" for key, value in counts.items()))
    if args.uncovered:
        print("\n".join(uncovered))


if __name__ == "__main__":
    main()
