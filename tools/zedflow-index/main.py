"""CocoIndex inventory for Zedflow cleanup planning."""

from __future__ import annotations

import hashlib
import pathlib
from typing import Iterator

import cocoindex as coco
from cocoindex.connectors import localfs

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = pathlib.Path(__file__).resolve().parent / "out"
SKIP_DIRS = {
    ".git",
    "target",
    ".target",
    ".cache",
    ".venv",
    "__pycache__",
    "node_modules",
    "legacy_pi_mono_code/pi-mono",
}
TEXT_EXTS = {
    ".rs",
    ".toml",
    ".md",
    ".json",
    ".jsonl",
    ".yml",
    ".yaml",
    ".sh",
    ".py",
    ".wit",
}


@coco.lifespan
def coco_lifespan(builder: coco.EnvironmentBuilder) -> Iterator[None]:
    """Keep CocoIndex state local to this utility."""
    builder.settings.db_path = pathlib.Path(__file__).resolve().parent / "cocoindex.db"
    yield


def _skip(rel: pathlib.Path) -> bool:
    return any(str(rel).startswith(d) for d in SKIP_DIRS)


def _kind(path: pathlib.Path) -> str:
    parts = path.parts
    if parts[:1] == ("src",):
        return "runtime"
    if parts[:1] == ("tests",) or parts[:1] in {("benches",), ("fuzz",)}:
        return "verification"
    if parts[:1] == ("docs",):
        return "docs"
    if parts[:1] == (".agents",) or parts[:1] == (".beads",):
        return "agentic-work-mgmt"
    if parts[:1] == ("scripts",) or path.name in {"install.sh", "uninstall.sh", "verify"}:
        return "utility"
    if parts[:1] == ("legacy_pi_mono_code",):
        return "legacy-reference"
    return "repo-root"


def _sample(path: pathlib.Path) -> str:
    try:
        text = path.read_text(errors="ignore")[:400]
    except OSError:
        return ""
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    return " | ".join(lines[:3])[:240]


@coco.fn(memo=True)
def build_inventory(root: pathlib.Path) -> str:
    rows: list[tuple[str, int, str, str, str]] = []
    counts: dict[str, int] = {}
    bytes_by_kind: dict[str, int] = {}

    for path in sorted(root.rglob("*")):
        rel = path.relative_to(root)
        if _skip(rel) or not path.is_file():
            continue
        try:
            size = path.stat().st_size
        except OSError:
            continue
        kind = _kind(rel)
        counts[kind] = counts.get(kind, 0) + 1
        bytes_by_kind[kind] = bytes_by_kind.get(kind, 0) + size
        if path.suffix in TEXT_EXTS:
            digest = hashlib.sha1(path.read_bytes()).hexdigest()[:10]
            rows.append((str(rel), size, kind, digest, _sample(path)))

    lines = ["# Zedflow CocoIndex inventory", "", f"Root: `{root}`", ""]
    lines.append("## Counts")
    for kind in sorted(counts):
        lines.append(f"- {kind}: {counts[kind]} files, {bytes_by_kind[kind]} bytes")
    lines.extend(["", "## Text files", "", "| path | bytes | kind | sha1 | sample |", "| --- | ---: | --- | --- | --- |"])
    for rel, size, kind, digest, sample in rows:
        safe_sample = sample.replace("|", "\\|")
        lines.append(f"| `{rel}` | {size} | {kind} | `{digest}` | {safe_sample} |")
    return "\n".join(lines) + "\n"


@coco.fn
async def app_main(root: pathlib.Path, outdir: pathlib.Path) -> None:
    inventory = build_inventory(root)
    localfs.declare_file(outdir / "inventory.md", inventory, create_parent_dirs=True)


app = coco.App(
    coco.AppConfig(name="zedflow-index"),
    app_main,
    root=ROOT,
    outdir=OUT,
)
