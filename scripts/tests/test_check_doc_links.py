import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import check_doc_links  # noqa: E402


class CheckDocLinksTest(unittest.TestCase):
    def test_slug_matches_github_style(self):
        self.assertEqual(check_doc_links.slug("3. Music analysis: music21 10.5.2"), "3-music-analysis-music21-1052")
        self.assertEqual(check_doc_links.slug("Candidates v47–v49 (`x`)"), "candidates-v47v49-x")
        self.assertEqual(check_doc_links.slug("開発"), "開発")

    def test_reports_missing_files_and_anchors_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "a.md").write_text("# A\n\n## Same\n\n## Same\n", encoding="utf-8")
            source = root / "b.md"
            source.write_text(
                "[ok](a.md#same-1) [self](#b) [web](https://example.com/x.md)\n"
                "[gone](missing.md) [bad](a.md#nope)\n"
                "```\n[fenced](missing.md)\n```\n`[code](missing.md)`\n# B\n",
                encoding="utf-8",
            )
            errors = check_doc_links.check(root, [source])
        self.assertEqual(len(errors), 2, errors)
        self.assertIn("missing target missing.md", errors[0])
        self.assertIn("missing anchor a.md#nope", errors[1])


if __name__ == "__main__":
    unittest.main()
