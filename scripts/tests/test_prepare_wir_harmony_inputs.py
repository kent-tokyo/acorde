"""Answer annotations are removed without altering score notes."""

from io import BytesIO
import hashlib
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_wir_harmony_inputs import count_harmony, prepare, strip_mxl


class PrepareWirInputsTests(unittest.TestCase):
    def test_removes_only_harmony_bytes_and_is_deterministic(self):
        xml = (b'<score-partwise><part><measure number="1">'
               b'<note><pitch><step>C</step></pitch><duration>1</duration></note>'
               b'<harmony><root><root-step>C</root-step></root></harmony>'
               b'<note><pitch><step>E</step></pitch><duration>1</duration></note>'
               b'</measure></part></score-partwise>')
        source = BytesIO()
        with zipfile.ZipFile(source, "w") as archive:
            archive.writestr("score.xml", xml)
        cleaned, removed = strip_mxl(source.getvalue())
        self.assertEqual(removed, 1)
        self.assertEqual(strip_mxl(source.getvalue()), (cleaned, 1))
        with zipfile.ZipFile(BytesIO(cleaned)) as archive:
            output_xml = archive.read("score.xml")
        self.assertEqual(count_harmony(output_xml), 0)
        self.assertEqual(output_xml, xml.replace(
            b'<harmony><root><root-step>C</root-step></root></harmony>', b''))

    def test_score_only_manifest_never_requires_gold_bytes(self):
        xml = b'<score-partwise><part><measure number="1"><note/></measure></part></score-partwise>'
        source = BytesIO()
        with zipfile.ZipFile(source, "w") as archive:
            archive.writestr("score.xml", xml)
        with TemporaryDirectory() as directory:
            source_root = Path(directory) / "source"
            output_root = Path(directory) / "clean"
            relative = Path("Corpus/example/score.mxl")
            path = source_root / relative
            path.parent.mkdir(parents=True)
            path.write_bytes(source.getvalue())
            selection = {"commit": "pinned", "works": [
                {"work": "Corpus/example", "score_path": str(relative)}]}
            hashes = {"commit": "pinned", "selection_sha256": "selection",
                      "files": [{"work": "Corpus/example",
                                 "score_sha256": hashlib.sha256(source.getvalue()).hexdigest()}]}
            result = prepare(selection, hashes, source_root, output_root)
            self.assertEqual(len(result["inputs"]), 1)
            self.assertNotIn("gold_sha256", result["inputs"][0])
            self.assertEqual((output_root / relative).read_bytes(), source.getvalue())


if __name__ == "__main__":
    unittest.main()
