# Migrating to acorde v0.6

The v0.6 release fixes MusicXML parsing for the first measure of each part. Clef, key signature,
and time signature values declared inside that measure's `<attributes>` element are now stored on
the measure after parsing, including non-default values.

This is a correctness fix and does not change the public function signatures. Existing consumers
may observe more accurate first-measure layout and playback metadata for MusicXML files that use
non-default initial attributes.
