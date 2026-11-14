"""Answer annotations are removed without altering score notes."""

from io import BytesIO
from pathlib import Path
import sys
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_wir_harmony_inputs import count_harmony, strip_mxl


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


if __name__ == "__main__":
    unittest.main()
