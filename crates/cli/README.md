# acorde-cli

Command-line conversion and inspection tool for acorde.

~~~bash
cargo install acorde-cli

acorde convert input.mid output.musicxml
acorde info input.musicxml
acorde validate input.musicxml
acorde report input.mei
acorde analyze input.musicxml
acorde extract --part 0 input.musicxml part.musicxml
~~~

Input supports .musicxml, .mxl, .mid/.midi, .abc, .mei, .mscz, and .mscx. Conversion output is
MusicXML or MIDI. info prints title, counts, tempo, time signature, and duration estimate; validate
exits with status 1 when structural errors are found. report emits the parsed score and structured
import diagnostics as JSON.
analyze emits deterministic chord, melodic-interval, and key-estimate results as JSON.

[Repository](https://github.com/kent-tokyo/acorde)
