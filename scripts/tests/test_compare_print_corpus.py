import importlib.util
import pathlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "compare_print_corpus.py"
spec = importlib.util.spec_from_file_location("compare_print_corpus", SCRIPT)
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)


class OutlinedTextScreenTest(unittest.TestCase):
    @unittest.skipUnless(pathlib.Path(compare.CHROME).exists(), "Chrome is not installed")
    def test_outlined_text_without_text_nodes_is_measured(self):
        svg = '''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 210 297">
          <g data-acorde-outlined-text="true" data-text="A &amp; B &lt; C" data-font-size="4" class="acorde-lyric">
            <path d="M 20 20 H 30 V 25 H 20 Z"/>
          </g><ellipse class="acorde-notehead" cx="25" cy="22" rx="4" ry="2"/>
          <g class="acorde-staff"><line x1="10" y1="22" x2="40" y2="22"/></g>
          <line class="acorde-stem" x1="24" y1="15" x2="24" y2="28"/>
          <circle class="margin-mark" cx="7" cy="30" r="1"/>
        </svg>'''
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            page = root / "page.svg"
            page.write_text(svg)
            result = compare.measure([page], compare.SELECTORS["acorde"], root / "screen.html")
        self.assertEqual(result["text_over_notation"], 1)
        self.assertEqual(result["text_notation_candidates"][0]["text"], "A & B < C")
        self.assertIsNotNone(result["lyric_font_mm"])
        self.assertEqual(len(result["lyric_staff_candidates"]), 1)
        self.assertEqual(len(result["text_curve_candidates"]), 1)
        self.assertTrue(any(c["className"] == "margin-mark" for c in result["margin_candidates"]))

    @unittest.skipUnless(pathlib.Path(compare.CHROME).exists(), "Chrome is not installed")
    def test_detects_displaced_syllable_in_the_same_measure_and_voice(self):
        svg = '''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 210 297">
          <g data-print-note-addr="0:0:7:0:0"><g class="acorde-lyric" data-acorde-outlined-text="true" data-text="keep" data-font-size="4"><path d="M 0 -3 H 5 V 0 H 0 Z" transform="translate(20 30)"/></g></g>
          <g data-print-note-addr="0:0:7:0:1"><g class="acorde-lyric" data-acorde-outlined-text="true" data-text="guide" data-font-size="4"><path d="M 0 -3 H 5 V 0 H 0 Z" transform="translate(40 34)"/></g></g>
          <g data-print-note-addr="0:0:7:1:0"><g class="acorde-lyric" data-acorde-outlined-text="true" data-text="other" data-font-size="4"><path d="M 0 -3 H 5 V 0 H 0 Z" transform="translate(60 40)"/></g></g>
        </svg>'''
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            page = root / "page.svg"
            page.write_text(svg)
            result = compare.measure([page], compare.SELECTORS["acorde"], root / "screen.html")
        self.assertEqual(len(result["lyric_baseline_candidates"]), 1)
        candidate = result["lyric_baseline_candidates"][0]
        self.assertEqual(candidate["address"], "0:0:7:0")
        self.assertEqual([s["text"] for s in candidate["syllables"]], ["keep", "guide"])


if __name__ == "__main__":
    unittest.main()
