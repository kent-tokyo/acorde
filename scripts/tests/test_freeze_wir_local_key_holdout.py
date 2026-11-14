import unittest

from freeze_wir_local_key_holdout import GROUPS, SOURCE, select


class LocalKeySelectionTests(unittest.TestCase):
    def test_selects_twelve_disjoint_works_from_tree_index(self):
        entries = []
        for prefix, skip, count, distinct in GROUPS:
            for index in range(skip + count):
                work = prefix + (f"Collection{index:02d}/1" if distinct else f"Work{index:02d}")
                for suffix in ("score.mxl", "analysis.txt"):
                    entries.append({"path": f"{work}/{suffix}", "type": "blob",
                                    "sha": f"blob-{index}-{suffix}"})
        commit = "a" * 40
        tree = {"sha": commit, "truncated": False, "tree": entries}
        record = {"sha": commit, "commit": {"tree": {"sha": "b" * 40}}}
        previous = [{"commit": commit, "source": SOURCE, "works": []} for _ in range(3)]
        result = select(tree, record, previous)
        self.assertEqual(len(result["works"]), 12)
        self.assertEqual(len({work["work"] for work in result["works"]}), 12)
        self.assertTrue(all(work["score_path"].endswith("score.mxl")
                            and work["gold_path"].endswith("analysis.txt")
                            for work in result["works"]))
        previous[0]["works"] = [{"work": result["works"][0]["work"]}]
        with self.assertRaisesRegex(ValueError, "prior work leaked"):
            select(tree, record, previous)


if __name__ == "__main__":
    unittest.main()
