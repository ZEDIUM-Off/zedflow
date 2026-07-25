#!/usr/bin/env python3
"""No-network checks for deterministic manifest closure."""
from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("manifest", ROOT / "tools/pi-port-swarm/manifest.py")
assert SPEC and SPEC.loader
manifest = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = manifest
SPEC.loader.exec_module(manifest)


class ManifestTests(unittest.TestCase):
    def fixture(self) -> Path:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        (root / "references/pi/packages/ai/src").mkdir(parents=True)
        (root / "references/pi/packages/ai/test").mkdir(parents=True)
        (root / "crates/zedflow-ai/src").mkdir(parents=True)
        (root / "crates/zedflow-ai/tests").mkdir(parents=True)
        (root / ".agents/port-manifests").mkdir(parents=True)
        (root / "references/pi/packages/ai/src/a.ts").write_text("export {};", encoding="utf-8")
        (root / "references/pi/packages/ai/test/a.test.ts").write_text("export {};", encoding="utf-8")
        (root / "crates/zedflow-ai/src/a.rs").write_text("", encoding="utf-8")
        (root / "crates/zedflow-ai/tests/a.rs").write_text("", encoding="utf-8")
        (root / ".agents/port-manifests/ai-src.tsv").write_text("src/a.ts\tcrates/zedflow-ai/src/a.rs\n", encoding="utf-8")
        (root / ".agents/port-manifests/ai-tests.tsv").write_text("test/a.test.ts\tcrates/zedflow-ai/tests/a.rs\n", encoding="utf-8")
        return root

    def test_closed_package_and_cli_check(self) -> None:
        root = self.fixture()
        self.assertEqual(manifest.report(root, "zedflow-ai")["status"], "valid")
        completed = subprocess.run([sys.executable, str(ROOT / "tools/pi-port-swarm/manifest.py"), "check", "--package", "zedflow-ai", "--source", str(root)], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(json.loads(completed.stdout)["status"], "valid")

    def test_reports_unlisted_duplicate_missing_and_arbitration(self) -> None:
        root = self.fixture()
        manifests = root / ".agents/port-manifests"
        (root / "references/pi/packages/ai/src/extra.ts").write_text("", encoding="utf-8")
        (root / "references/pi/packages/ai/src/unlisted.ts").write_text("", encoding="utf-8")
        (manifests / "ai-src.tsv").write_text("src/a.ts\tcrates/zedflow-ai/src/missing.rs\nsrc/a.ts\tcrates/zedflow-ai/src/missing.rs\n", encoding="utf-8")
        (manifests / "exceptions.tsv").write_text("ai\tsrc/extra.ts\tdependency-arbitration\t\tneeds approved replacement\n", encoding="utf-8")
        value = manifest.report(root, "zedflow-ai")
        self.assertEqual(value["status"], "blocked")
        self.assertEqual(value["packages"]["zedflow-ai"]["dependency_arbitration"], ["src/extra.ts"])
        self.assertEqual(value["packages"]["zedflow-ai"]["unlisted"], ["src/unlisted.ts"])
        self.assertTrue(any("duplicate mapping" in error for error in value["errors"]))
        self.assertTrue(any("missing target" in error for error in value["errors"]))

    def test_coding_agent_highlight_declaration_is_type_only(self) -> None:
        value = manifest.report(ROOT, "zedflow-coding-agent")
        row = next(row for row in value["packages"]["zedflow-coding-agent"]["exceptions"] if row["source"] == "src/utils/highlight-js-lib-index.d.ts")
        self.assertEqual(row["disposition"], "type-only")
        self.assertEqual(row["target"], "")

    def test_invalid_disposition_is_reported(self) -> None:
        root = self.fixture()
        (root / ".agents/port-manifests/exceptions.tsv").write_text("ai\tsrc/a.ts\tunsupported\t\tevidence\n", encoding="utf-8")
        value = manifest.report(root, "zedflow-ai")
        self.assertTrue(any("invalid disposition" in error for error in value["errors"]))

    def test_root_declaration_and_incomplete_consolidation_block_closure(self) -> None:
        root = self.fixture()
        declaration = root / "references/pi/packages/ai/provider.d.ts"
        declaration.write_text("export {};", encoding="utf-8")
        exceptions = root / ".agents/port-manifests/exceptions.tsv"
        exceptions.write_text("ai\tprovider.d.ts\tconsolidated\tcrates/zedflow-ai/src/a.rs\tone source is not consolidation\n", encoding="utf-8")
        value = manifest.report(root, "zedflow-ai")
        self.assertEqual(value["status"], "blocked")
        self.assertTrue(any("at least two sources" in error for error in value["errors"]))

    def test_disposition_target_must_stay_in_crates(self) -> None:
        root = self.fixture()
        (root / "references/pi/packages/ai/src/platform.ts").write_text("", encoding="utf-8")
        (root / ".agents/port-manifests/exceptions.tsv").write_text("ai\tsrc/platform.ts\tplatform-specific\t../outside.rs\tevidence\n", encoding="utf-8")
        value = manifest.report(root, "zedflow-ai")
        self.assertTrue(any("escapes crates" in error for error in value["errors"]))

    def test_revision_target_check_uses_git_tree(self) -> None:
        self.assertTrue(manifest.target_exists(ROOT, "crates/zedflow-ai/src/lib.rs", "HEAD"))
        self.assertFalse(manifest.target_exists(ROOT, "crates/zedflow-ai/src/not-present.rs", "HEAD"))


if __name__ == "__main__":
    unittest.main()
