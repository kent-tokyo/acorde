"""Focused checks for the independent written note-span reader."""

import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare_musicxml_onsets import match_written_durations, source_attacks, source_spans


def mxl(path: Path, body: str) -> None:
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(
            "META-INF/container.xml",
            '<container><rootfiles><rootfile full-path="score.xml"/></rootfiles></container>',
        )
        archive.writestr("score.xml", f'<score-partwise><part id="P1">{body}</part></score-partwise>')


class SourceSpanTests(unittest.TestCase):
    def test_tie_across_voice_change_and_marked_note(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "score.mxl"
            mxl(path, """
                <measure><attributes><divisions>1</divisions></attributes>
                  <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                    <voice>1</voice><staff>1</staff><tie type="start"/></note>
                </measure>
                <measure>
                  <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                    <voice>2</voice><staff>1</staff><tie type="stop"/></note>
                  <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration>
                    <voice>2</voice><staff>1</staff>
                    <notations><articulations><staccato/></articulations></notations></note>
                </measure>
            """)
            spans = source_spans(path)
            self.assertEqual([(s.start, s.end, s.pitch) for s in spans],
                             [(0.0, 2.0, 60), (2.0, 3.0, 62)])
            self.assertEqual(source_attacks(path)[0].total(), len(spans))
            self.assertEqual(spans[1].release_caveats, {"articulations"})
            report = match_written_durations(
                spans, {(0.0, 60): [(0.0, 2.0, False)], (2.0, 62): [(0.0, 0.5, False)]},
                0.002,
            )
            self.assertEqual(report["strict_eligible_matched_spans"], 1)
            self.assertEqual(report["strict_written_duration_matches"], 1)

    def test_overlapping_unison_has_ambiguous_note_off(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "score.mxl"
            mxl(path, """
                <measure><attributes><divisions>1</divisions></attributes>
                  <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                    <voice>1</voice></note>
                  <backup><duration>1</duration></backup>
                  <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
                    <voice>2</voice></note>
                </measure>
            """)
            spans = source_spans(path)
            self.assertEqual(len(spans), 2)
            self.assertTrue(all("overlapping_same_pitch" in s.release_caveats for s in spans))


if __name__ == "__main__":
    unittest.main()
