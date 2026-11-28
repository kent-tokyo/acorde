"""Focused checks for the release version-marker consistency check."""

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_release_consistency import check, main


def write_repo(
    root: Path,
    *,
    version: str = "2.1.0",
    dep_version: str = "2.1.0",
    release: str = "2.1.0",
    base_release: str = "2.0.1",
    changelog_top: str = "2.1.0",
    roadmap: str = "2.1.0",
) -> None:
    (root / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crates/io"]\n\n'
        f'[workspace.package]\nversion = "{version}"\nedition = "2024"\n',
        encoding="utf-8",
    )
    crate = root / "crates" / "io"
    crate.mkdir(parents=True)
    (crate / "Cargo.toml").write_text(
        '[package]\nname = "acorde-io"\nversion.workspace = true\n\n'
        "[dependencies]\n"
        f'acorde-core = {{ path = "../core", version = "{dep_version}" }}\n'
        'serde = { version = "1", features = ["derive"] }\n',
        encoding="utf-8",
    )
    (root / "docs").mkdir()
    (root / "docs" / "scorecard.json").write_text(
        json.dumps({"schema_version": 1, "release": release, "base_release": base_release}),
        encoding="utf-8",
    )
    (root / "CHANGELOG.md").write_text(
        "# Changelog\n\n## [Unreleased]\n\n"
        f"## {changelog_top} - 2026-12-01\n\n- change\n\n"
        "## 2.0.1 - 2026-11-23\n\n- fix\n",
        encoding="utf-8",
    )
    (root / "ROADMAP.md").write_text(
        f"# Roadmap\n\nVersion {roadmap} includes a score model.\n", encoding="utf-8"
    )


class ReleaseConsistencyTests(unittest.TestCase):
    def run_check(self, **markers: str) -> list[str]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_repo(root, **markers)
            return check(root)

    def test_consistent_markers_pass(self) -> None:
        self.assertEqual(self.run_check(), [])

    def test_stale_scorecard_release_fails(self) -> None:
        errors = self.run_check(release="2.0.1", base_release="2.0.0")
        self.assertTrue(any("scorecard.json: release" in error for error in errors))
        self.assertTrue(any("base_release" in error for error in errors))

    def test_internal_dependency_version_fails(self) -> None:
        errors = self.run_check(dep_version="2.0.1")
        self.assertEqual(len(errors), 1)
        self.assertIn("acorde-core requires 2.0.1", errors[0])

    def test_changelog_without_release_entry_fails(self) -> None:
        errors = self.run_check(changelog_top="2.0.2")
        self.assertTrue(any("CHANGELOG.md" in error for error in errors))

    def test_stale_roadmap_baseline_fails(self) -> None:
        errors = self.run_check(roadmap="2.0.1")
        self.assertEqual(errors, ["ROADMAP.md: baseline names 2.0.1, workspace is 2.1.0"])

    def test_expected_tag_version_mismatch_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_repo(root)
            self.assertEqual(main(["--root", str(root), "--expect", "2.1.0"]), 0)
            self.assertEqual(main(["--root", str(root), "--expect", "2.1.1"]), 1)


if __name__ == "__main__":
    unittest.main()
