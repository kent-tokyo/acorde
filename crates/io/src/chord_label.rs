//! Chord names as text (`C#m7`, `F/A`, `G7b9`), shared by the MEI and Guitar Pro importers.

use acorde_core::{ChordDegree, ChordSymbol};

/// A chord symbol from a compact chord name: root, kind suffix with added/altered/omitted
/// degrees, and an optional slash bass. `None` when the text is not a chord name.
pub(crate) fn parse_chord_label(value: &str) -> Option<ChordSymbol> {
    // Typeset spellings (♭, ♯) and grouping marks (`7(b9, #11)`) reduce to the compact form.
    let value: String = value
        .trim()
        .replace('♭', "b")
        .replace('♯', "#")
        .chars()
        .filter(|ch| !matches!(ch, '(' | ')' | ',' | ' ' | '♮'))
        .collect();
    let value = value.as_str();
    let (label, bass) = value
        .split_once('/')
        .map_or((value, None), |(label, bass)| (label, Some(bass)));
    let mut chars = label.chars();
    let step = chars.next()?.to_ascii_uppercase();
    if !matches!(step, 'A'..='G') {
        return None;
    }
    let accidental = match chars.next() {
        Some('#') => "#",
        Some('b') => "b",
        Some(_) => "",
        None => "",
    };
    let consumed = if accidental.is_empty() { 1 } else { 2 };
    let suffix = canonical_suffix(&label[consumed..]);
    let suffix = suffix.as_str();
    // A whole suffix naming a kind (`aug7`, `13`, `mMaj7`) wins over quality plus degrees.
    let (kind, degrees) = match ChordSymbol::kind_for_suffix(suffix) {
        Some(kind) => (kind, Vec::new()),
        None => parse_compact_chord_suffix(suffix)?,
    };
    let kind = match kind {
        "" | "maj" => "major",
        "m" | "min" => "minor",
        "7" => "dominant",
        "maj7" => "major-seventh",
        "m7" | "min7" => "minor-seventh",
        "dim" => "diminished",
        "dim7" => "diminished-seventh",
        "aug" => "augmented",
        "sus2" => "suspended-second",
        "sus4" => "suspended-fourth",
        "m7b5" => "half-diminished",
        "6" => "major-sixth",
        "m6" => "minor-sixth",
        other => other,
    };
    let bass = bass.map(str::trim).filter(|value| !value.is_empty());
    if let Some(bass) = bass
        && !is_note_name(bass)
    {
        return None;
    }
    Some(ChordSymbol {
        root: format!("{step}{accidental}"),
        kind: kind.to_string(),
        bass: bass.map(ToString::to_string),
        placement: None,
        extender: false,
        harmonic_degree: None,
        harmony_function: None,
        harmony_type: None,
        chord_ref: None,
        range_end: None,
        degrees,
    })
}

/// Common alternative spellings of a chord suffix in the compact form the parser reads: `Maj7`,
/// `M7` and `Δ` for `maj7`, `-` and `min` for `m`, `+` for `aug`, `°` for `dim`, `ø` for
/// `m7b5`.
fn canonical_suffix(suffix: &str) -> String {
    const WHOLE: &[(&str, &str)] = &[("M", ""), ("sus", "sus4"), ("7+", "aug7"), ("+7", "aug7")];
    if let Some((_, canonical)) = WHOLE.iter().find(|(alias, _)| *alias == suffix) {
        return (*canonical).to_string();
    }
    const PREFIXES: &[(&str, &str)] = &[
        ("mMaj7", "mMaj7"),
        ("minMaj7", "mMaj7"),
        ("mM7", "mMaj7"),
        ("mΔ7", "mMaj7"),
        ("-Δ7", "mMaj7"),
        ("-Δ", "mMaj7"),
        ("Maj13", "maj13"),
        ("Maj11", "maj11"),
        ("Maj9", "maj9"),
        ("Maj7", "maj7"),
        ("M13", "maj13"),
        ("M9", "maj9"),
        ("M7", "maj7"),
        ("Δ9", "maj9"),
        ("Δ7", "maj7"),
        ("Δ", "maj7"),
        ("ma7", "maj7"),
        ("Maj", "maj"),
        ("ø7", "m7b5"),
        ("ø", "m7b5"),
        ("°7", "dim7"),
        ("°", "dim"),
        ("-7", "m7"),
        ("-", "m"),
        ("min", "m"),
        ("mi", "m"),
        ("+", "aug"),
    ];
    PREFIXES
        .iter()
        .find_map(|(alias, canonical)| {
            suffix
                .strip_prefix(alias)
                .map(|rest| format!("{canonical}{rest}"))
        })
        .unwrap_or_else(|| suffix.to_string())
}

fn parse_compact_chord_suffix(suffix: &str) -> Option<(&str, Vec<ChordDegree>)> {
    let qualities = [
        ("maj7", "major-seventh"),
        ("m7b5", "half-diminished"),
        ("dim7", "diminished-seventh"),
        ("sus2", "suspended-second"),
        ("sus4", "suspended-fourth"),
        ("m6", "minor-sixth"),
        ("maj", "major"),
        ("min", "minor"),
        ("dim", "diminished"),
        ("aug", "augmented"),
        ("m", "minor"),
        ("7", "dominant"),
        ("6", "major-sixth"),
        ("", "major"),
    ];
    let (quality, kind) = qualities
        .iter()
        .find_map(|(prefix, kind)| suffix.strip_prefix(prefix).map(|rest| (rest, *kind)))?;
    let mut degrees = Vec::new();
    let mut rest = quality;
    while !rest.is_empty() {
        let (degree_kind, tail) = if let Some(tail) = rest.strip_prefix("add") {
            ("add", tail)
        } else if let Some(tail) = rest.strip_prefix("no") {
            ("subtract", tail)
        } else {
            ("alter", rest)
        };
        let accidental_len = tail
            .chars()
            .take_while(|ch| matches!(ch, '#' | 'b'))
            .count();
        let (accidentals, number) = tail.split_at(accidental_len);
        let number_len = number.chars().take_while(char::is_ascii_digit).count();
        if number_len == 0 {
            return None;
        }
        let value = number[..number_len].parse::<u8>().ok().filter(|v| *v > 0)?;
        let alter = match accidentals {
            "" => 0,
            "#" => 1,
            "##" => 2,
            "b" => -1,
            "bb" => -2,
            _ => return None,
        };
        degrees.push(ChordDegree {
            value,
            alter,
            kind: degree_kind.to_string(),
        });
        rest = &number[number_len..];
    }
    Some((kind, degrees))
}

fn is_note_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(step) = chars.next() else {
        return false;
    };
    if !matches!(step.to_ascii_uppercase(), 'A'..='G') {
        return false;
    }
    matches!(chars.next(), None | Some('#') | Some('b')) && chars.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::parse_chord_label;

    #[test]
    fn alternative_spellings_parse() {
        for (label, root, kind) in [
            ("C+", "C", "augmented"),
            ("DMaj7", "D", "major-seventh"),
            ("D7+", "D", "augmented-seventh"),
            ("DmMaj7", "D", "major-minor"),
            ("B♭6", "Bb", "major-sixth"),
            ("F♯ø7", "F#", "half-diminished"),
            ("E°7", "E", "diminished-seventh"),
            ("A-7", "A", "minor-seventh"),
            ("GΔ", "G", "major-seventh"),
            ("Cmin", "C", "minor"),
            ("Dsus", "D", "suspended-fourth"),
        ] {
            let chord = parse_chord_label(label).unwrap_or_else(|| panic!("{label} parses"));
            assert_eq!(
                (chord.root.as_str(), chord.kind.as_str()),
                (root, kind),
                "{label}"
            );
        }
        let altered = parse_chord_label("G7(b9, #11)/B").expect("altered chord parses");
        assert_eq!(altered.kind, "dominant");
        assert_eq!(altered.degrees.len(), 2);
        assert_eq!(altered.bass.as_deref(), Some("B"));
    }
}
