import unittest

from diagnose_wir_local_keys import MAJOR, key_pitch_class_and_mode, predict


class LocalKeyDiagnosticTests(unittest.TestCase):
    def test_enharmonic_spelling_follows_written_signature_on_profile_tie(self):
        c_sharp_profile = [MAJOR[(pitch_class - 1) % 12] for pitch_class in range(12)]
        self.assertEqual(predict([c_sharp_profile], 0, [7]), ["C# major"])
        self.assertEqual(predict([c_sharp_profile], 0, [-5]), ["D- major"])
        self.assertEqual(key_pitch_class_and_mode("C# major"),
                         key_pitch_class_and_mode("D- major"))
        self.assertEqual(key_pitch_class_and_mode("C## major"),
                         key_pitch_class_and_mode("D major"))


if __name__ == "__main__":
    unittest.main()
