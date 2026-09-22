use super::super::{Error, MAX_INPUT_BYTES};
use acorde_core::{
    Articulation, BeamState, Clef, Duration, Note, NoteHead, Pitch, Score, Staff, Step,
};
use std::fmt::Write;
use std::io::{Cursor, Write as IoWrite};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// Report canonical score fields that the deliberately bounded MSCX serializer cannot emit.
pub fn export_loss_diagnostics(score: &Score) -> Vec<crate::Diagnostic> {
    const MAX_DIAGNOSTICS: usize = 1_024;
    let mut diagnostics = Vec::new();
    let mut push = |path: String, value: String, reason: &str| {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            return;
        }
        let mut diagnostic = crate::Diagnostic::warning("mscx.export-unsupported-field", reason);
        diagnostic.source_location = Some(path);
        diagnostic.preserved_value = Some(value);
        diagnostics.push(diagnostic);
    };

    if !score.chord_definitions.is_empty() {
        push(
            "/score/chord-definitions".to_string(),
            score.chord_definitions.len().to_string(),
            "MSCX subset export does not emit reusable chord definitions",
        );
    }
    if !score.part_groups.is_empty() {
        push(
            "/score/part-groups".to_string(),
            score.part_groups.len().to_string(),
            "MSCX subset export does not emit score-level part groups",
        );
    }

    for (part_index, part) in score.parts.iter().enumerate() {
        let part_path = format!("/score/part/{}", part_index + 1);
        for (field, present, value, reason) in [
            (
                "midi-pitch-bends",
                !part.midi_pitch_bends.is_empty(),
                part.midi_pitch_bends.len().to_string(),
                "MSCX subset export does not emit MIDI pitch-bend events",
            ),
            (
                "midi-control-changes",
                !part.midi_control_changes.is_empty(),
                part.midi_control_changes.len().to_string(),
                "MSCX subset export does not emit MIDI control-change events",
            ),
            (
                "midi-program-changes",
                !part.midi_program_changes.is_empty(),
                part.midi_program_changes.len().to_string(),
                "MSCX subset export does not emit MIDI program-change events",
            ),
            (
                "midi-aftertouch",
                !part.midi_aftertouch.is_empty(),
                part.midi_aftertouch.len().to_string(),
                "MSCX subset export does not emit MIDI aftertouch events",
            ),
            (
                "percussion-instruments",
                !part.percussion_instruments.is_empty(),
                part.percussion_instruments.len().to_string(),
                "MSCX subset export does not emit percussion instrument declarations",
            ),
            (
                "staff-groups",
                !part.staff_groups.is_empty(),
                part.staff_groups.len().to_string(),
                "MSCX subset export does not emit staff-group metadata",
            ),
        ] {
            if present {
                push(format!("{part_path}/{field}"), value, reason);
            }
        }
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let staff_path = format!("{part_path}/staff/{}", staff_index + 1);
            if staff.transpose_semitones != 0 {
                push(
                    format!("{staff_path}/transpose-semitones"),
                    staff.transpose_semitones.to_string(),
                    "MSCX subset export does not emit staff transposition",
                );
            }
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                let measure_path = format!("{staff_path}/measure/{}", measure_index + 1);
                for (field, present, value, reason) in [
                    (
                        "tempo-text",
                        measure.tempo_text.is_some(),
                        measure.tempo_text.clone().unwrap_or_default(),
                        "MSCX subset export does not emit tempo text",
                    ),
                    (
                        "rehearsal",
                        measure.rehearsal.is_some(),
                        measure.rehearsal.clone().unwrap_or_default(),
                        "MSCX subset export does not emit rehearsal marks",
                    ),
                    (
                        "navigation",
                        measure.navigation.is_some(),
                        measure.navigation.clone().unwrap_or_default(),
                        "MSCX subset export does not emit navigation text",
                    ),
                    (
                        "expression-text",
                        measure.expression_text.is_some(),
                        measure.expression_text.clone().unwrap_or_default(),
                        "MSCX subset export does not emit expression text",
                    ),
                    (
                        "figured-bass",
                        !measure.figured_bass.is_empty(),
                        measure.figured_bass.len().to_string(),
                        "MSCX subset export does not emit figured bass",
                    ),
                    (
                        "tablature-change",
                        measure.tablature_change.is_some(),
                        measure
                            .tablature_change
                            .as_ref()
                            .map(|change| {
                                format!(
                                    "lines={},tuning_midi={:?},capo={}",
                                    change.lines, change.tuning_midi, change.capo
                                )
                            })
                            .unwrap_or_default(),
                        "MSCX subset export does not emit measure-local tablature tuning or capo changes",
                    ),
                    (
                        "tempo-ramp-to",
                        measure.tempo_ramp_to.is_some(),
                        measure
                            .tempo_ramp_to
                            .map(|target_bpm| target_bpm.to_string())
                            .unwrap_or_default(),
                        "MSCX subset export does not emit measure-local tempo ramps",
                    ),
                ] {
                    if present {
                        push(format!("{measure_path}/{field}"), value, reason);
                    }
                }
                if measure.multi_rest_count.is_some() || measure.system_break || measure.page_break
                {
                    push(
                        format!("{measure_path}/layout"),
                        "present".to_string(),
                        "MSCX subset export does not emit multi-rest or explicit layout breaks",
                    );
                }
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        let note_path = format!(
                            "{measure_path}/voice/{}/note/{}",
                            voice_index + 1,
                            note_index + 1
                        );
                        if note_has_unsupported_fields(note) {
                            push(
                                note_path,
                                "present".to_string(),
                                "MSCX subset export does not emit every note notation field",
                            );
                        }
                    }
                }
            }
        }
    }
    diagnostics
}

fn note_has_unsupported_fields(note: &Note) -> bool {
    note.pitches
        .iter()
        .any(|pitch| pitch.microtone_cents != 0 && !matches!(pitch.microtone_cents, -50 | 50))
        || note.articulations.iter().any(|articulation| {
            !matches!(articulation, Articulation::Tremolo(_))
                && articulation_subtype(articulation).is_none()
        })
        || note.hairpin_start.is_some()
        || note.hairpin_end
        || note.ottava_start.is_some()
        || note.ottava_end
        || note.pedal_start
        || note.pedal_end
        || note.slur_start
        || note.slur_end
        || note.glissando_start
        || note.glissando_end
        || note.cross_staff.is_some()
        || note.is_cue
        || note.trill_line_start
        || note.trill_line_end
        || note.guitar_bend_alter_cents.is_some()
        || !note.guitar_bend_curve.is_empty()
        || !matches!(
            note.beam,
            BeamState::None | BeamState::Begin | BeamState::Continue | BeamState::End
        )
}

/// Serialize the bounded canonical score subset as MuseScore 3-compatible MSCX.
///
/// This is intentionally a canonical subset serializer: it emits score structure, voices,
/// durations, pitches, tablature positions, common note techniques, lyrics, and basic measure
/// metadata. Unsupported model fields remain outside this function's lossless claim and should
/// be surfaced by the caller's export diagnostics.
pub fn serialize_mscx(score: &Score) -> Result<String, Error> {
    if score.parts.is_empty() {
        return Err(Error::Empty);
    }
    let mut xml = String::with_capacity(4096);
    xml.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="3.02"><Score>"#,
    );
    for (name, value) in [
        ("workTitle", score.metadata.title.as_str()),
        ("composer", score.metadata.composer.as_str()),
        ("lyricist", score.metadata.lyricist.as_str()),
        ("copyright", score.metadata.copyright.as_str()),
        ("workNumber", score.metadata.work_number.as_str()),
        ("movementTitle", score.metadata.movement_title.as_str()),
    ] {
        if !value.is_empty() {
            write!(xml, "<metaTag name=\"{name}\">{}</metaTag>", escape(value))
                .map_err(fmt_error)?;
        }
    }
    for part in &score.parts {
        xml.push_str("<Part>");
        for staff_index in 0..part.staves.len() {
            write!(xml, "<Staff id=\"{}\"/>", staff_index + 1).map_err(fmt_error)?;
        }
        write!(xml, "<trackName>{}</trackName>", escape(&part.name)).map_err(fmt_error)?;
        if !part.short_name.is_empty() {
            write!(xml, "<shortName>{}</shortName>", escape(&part.short_name))
                .map_err(fmt_error)?;
        }
        write!(
            xml,
            "<Instrument><Channel><program value=\"{}\"/><midiChannel>{}</midiChannel></Channel></Instrument>",
            part.midi_program.min(127),
            part.midi_channel.min(15)
        )
        .map_err(fmt_error)?;
        xml.push_str("</Part>");
    }
    let mut staff_id = 1usize;
    let mut score_texts_pending = Some(score.texts.as_slice());
    for part in &score.parts {
        for staff in &part.staves {
            let score_texts = score_texts_pending.take().unwrap_or_default();
            write_staff(&mut xml, staff_id, staff, score_texts)?;
            staff_id = staff_id.saturating_add(1);
        }
    }
    xml.push_str("</Score></museScore>");
    if xml.len() > MAX_INPUT_BYTES {
        return Err(Error::TooLarge(xml.len()));
    }
    Ok(xml)
}

/// Serialize the same bounded subset into an in-memory MuseScore `.mscz` archive.
pub fn serialize_mscz(score: &Score) -> Result<Vec<u8>, Error> {
    let xml = serialize_mscx(score)?;
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    archive
        .start_file("score.mscx", SimpleFileOptions::default())
        .map_err(|error| Error::Zip(error.to_string()))?;
    archive
        .write_all(xml.as_bytes())
        .map_err(|error| Error::Zip(error.to_string()))?;
    archive
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|error| Error::Zip(error.to_string()))
}

fn write_staff(
    xml: &mut String,
    id: usize,
    staff: &Staff,
    score_texts: &[acorde_core::StyledText],
) -> Result<(), Error> {
    write!(xml, "<Staff id=\"{id}\">").map_err(fmt_error)?;
    if !score_texts.is_empty() {
        xml.push_str("<VBox>");
        for styled in score_texts {
            write_styled_text(xml, styled)?;
        }
        xml.push_str("</VBox>");
    }
    if let Some(tab) = &staff.tablature {
        write!(
            xml,
            "<StaffType group=\"tab\"><lines>{}</lines><StringData>",
            tab.lines
        )
        .map_err(fmt_error)?;
        for tuning in tab.tuning_midi.iter().take(tab.lines as usize) {
            write!(xml, "<string>{tuning}</string>").map_err(fmt_error)?;
        }
        xml.push_str("</StringData></StaffType>");
    } else if staff.presentation.lines != 5 {
        write!(
            xml,
            "<StaffType group=\"pitched\"><lines>{}</lines></StaffType>",
            staff.presentation.lines
        )
        .map_err(fmt_error)?;
    }
    for (measure_index, measure) in staff.measures.iter().enumerate() {
        let number = if measure.number == 0 {
            measure_index as u32 + 1
        } else {
            measure.number
        };
        write!(xml, "<Measure number=\"{number}\">").map_err(fmt_error)?;
        if let Some(key) = &measure.key_sig {
            write!(
                xml,
                "<KeySig><accidental>{}</accidental><mode>{}</mode></KeySig>",
                key.fifths,
                escape(&key.mode)
            )
            .map_err(fmt_error)?;
        }
        if let Some(time) = &measure.time_sig {
            write!(
                xml,
                "<TimeSig><sigN>{}</sigN><sigD>{}</sigD></TimeSig>",
                time.numerator, time.denominator
            )
            .map_err(fmt_error)?;
        }
        if let Some(clef) = &measure.clef {
            write!(
                xml,
                "<Clef><concertClefType>{}</concertClefType></Clef>",
                clef_name(clef)
            )
            .map_err(fmt_error)?;
        }
        if let Some(bpm) = measure.tempo {
            write!(
                xml,
                "<Tempo><tempo>{:.6}</tempo></Tempo>",
                bpm as f64 / 60.0
            )
            .map_err(fmt_error)?;
        }
        for styled in &measure.texts {
            write_styled_text(xml, styled)?;
        }
        for (voice_index, voice) in measure.voices.iter().enumerate() {
            if voice.is_empty() {
                continue;
            }
            if voice_index > 0 {
                write!(xml, "<voice>{}", voice_index + 1).map_err(fmt_error)?;
            }
            for note in voice {
                write_note(xml, note)?;
            }
            if voice_index > 0 {
                xml.push_str("</voice>");
            }
        }
        if matches!(measure.barline_right, acorde_core::Barline::RepeatEnd) {
            xml.push_str("<endRepeat/>");
        }
        xml.push_str("</Measure>");
    }
    xml.push_str("</Staff>");
    Ok(())
}

fn write_styled_text(xml: &mut String, styled: &acorde_core::StyledText) -> Result<(), Error> {
    let has_offset = styled.offset_x.is_some() || styled.offset_y.is_some();
    let element = if has_offset { "StaffText" } else { "Text" };
    write!(xml, "<{element}>").map_err(fmt_error)?;
    if has_offset {
        xml.push_str("<offset");
        if let Some(x) = styled.offset_x {
            write!(xml, " x=\"{x:.6}\"").map_err(fmt_error)?;
        }
        if let Some(y) = styled.offset_y {
            write!(xml, " y=\"{y:.6}\"").map_err(fmt_error)?;
        }
        xml.push_str("/>");
    }
    write!(xml, "<style>{}</style>", text_style_name(&styled.style)).map_err(fmt_error)?;
    write!(xml, "<text>{}</text></{element}>", escape(&styled.text)).map_err(fmt_error)
}

fn write_note(xml: &mut String, note: &Note) -> Result<(), Error> {
    if note.is_rest {
        write!(
            xml,
            "<Rest><durationType>{}</durationType>{}</Rest>",
            duration_name(&note.duration),
            dots(note.dot_count)
        )
        .map_err(fmt_error)?;
        return Ok(());
    }
    if note.pitches.is_empty() {
        return Ok(());
    }
    if let Some(chord) = &note.chord_symbol {
        write!(
            xml,
            "<Harmony><name>{}</name></Harmony>",
            escape(&chord.display_text())
        )
        .map_err(fmt_error)?;
    }
    if let Some(dynamic) = &note.dynamic {
        write!(
            xml,
            "<Dynamic><subtype>{}</subtype><velocity>{}</velocity></Dynamic>",
            dynamic.to_musicxml_str(),
            dynamic.to_velocity()
        )
        .map_err(fmt_error)?;
    }
    write!(
        xml,
        "<Chord><durationType>{}</durationType>{}",
        duration_name(&note.duration),
        dots(note.dot_count)
    )
    .map_err(fmt_error)?;
    if let Some(tuplet) = &note.tuplet {
        write!(
            xml,
            "<Tuplet><actualNotes>{}</actualNotes><normalNotes>{}</normalNotes></Tuplet>",
            tuplet.actual_notes, tuplet.normal_notes
        )
        .map_err(fmt_error)?;
    }
    if let Some(stem_up) = note.stem_up {
        write!(
            xml,
            "<stemDirection>{}</stemDirection>",
            if stem_up { "up" } else { "down" }
        )
        .map_err(fmt_error)?;
    }
    if let Some(mode) = beam_mode(note.beam) {
        write!(xml, "<BeamMode>{mode}</BeamMode>").map_err(fmt_error)?;
    }
    if let Some(arpeggiate) = note.arpeggiate {
        write!(
            xml,
            "<Arpeggio><subtype>{}</subtype></Arpeggio>",
            if arpeggiate { "Up" } else { "Down" }
        )
        .map_err(fmt_error)?;
    }
    for (pitch_index, pitch) in note.pitches.iter().enumerate() {
        write!(
            xml,
            "<Note><pitch>{}</pitch><tpc>{}</tpc>",
            pitch.to_midi().clamp(0, 127),
            pitch_tpc(pitch)
        )
        .map_err(fmt_error)?;
        if note.note_head != NoteHead::Normal {
            write!(xml, "<head>{}</head>", note_head_name(&note.note_head)).map_err(fmt_error)?;
        }
        if let Some(subtype) = microtone_subtype(pitch.microtone_cents) {
            write!(xml, "<Accidental><subtype>{subtype}</subtype></Accidental>")
                .map_err(fmt_error)?;
        }
        if let Some(position) = note
            .tab_positions
            .get(pitch_index)
            .or(note.tab_position.as_ref())
        {
            write!(
                xml,
                "<string>{}</string><fret>{}</fret>",
                position.string, position.fret
            )
            .map_err(fmt_error)?;
        }
        for fingering in &note.fingerings {
            write!(xml, "<Fingering>{fingering}</Fingering>").map_err(fmt_error)?;
        }
        match note.guitar_technique {
            Some(acorde_core::GuitarTechnique::Bend) => xml.push_str("<Bend/>"),
            Some(acorde_core::GuitarTechnique::Slide) => xml.push_str("<Slide/>"),
            Some(acorde_core::GuitarTechnique::HammerOn) => xml.push_str("<HammerOn/>"),
            Some(acorde_core::GuitarTechnique::PullOff) => xml.push_str("<PullOff/>"),
            None => {}
        }
        if note.tie_start || note.tie_end {
            xml.push_str("<Spanner type=\"Tie\">");
            if note.tie_end {
                xml.push_str("<prev/>");
            }
            if note.tie_start {
                xml.push_str("<next/>");
            }
            xml.push_str("</Spanner>");
        }
        xml.push_str("</Note>");
    }
    for articulation in &note.articulations {
        match articulation {
            Articulation::Tremolo(level) => {
                write!(
                    xml,
                    "<Tremolo><subtype>c{}</subtype></Tremolo>",
                    1u32 << (level + 1)
                )
                .map_err(fmt_error)?;
            }
            articulation => {
                if let Some(subtype) = articulation_subtype(articulation) {
                    write!(
                        xml,
                        "<Articulation><subtype>{subtype}</subtype></Articulation>"
                    )
                    .map_err(fmt_error)?;
                }
            }
        }
    }
    if let Some(lyric) = &note.lyric {
        write!(
            xml,
            "<Lyrics><syllabic>{}</syllabic><text>{}</text></Lyrics>",
            escape(&lyric.syllabic),
            escape(&lyric.text)
        )
        .map_err(fmt_error)?;
    }
    xml.push_str("</Chord>");
    Ok(())
}

fn duration_name(duration: &Duration) -> &'static str {
    match duration {
        Duration::Whole => "whole",
        Duration::Half => "half",
        Duration::Quarter => "quarter",
        Duration::Eighth => "eighth",
        Duration::Sixteenth => "16th",
        Duration::ThirtySecond => "32nd",
        Duration::SixtyFourth => "64th",
    }
}

fn dots(count: u8) -> String {
    if count == 0 {
        String::new()
    } else {
        format!("<dots>{count}</dots>")
    }
}

fn clef_name(clef: &Clef) -> &'static str {
    match clef {
        Clef::Treble => "G",
        Clef::Bass => "F",
        Clef::Alto | Clef::Tenor => "C",
        Clef::Percussion => "PERC",
    }
}

fn text_style_name(style: &acorde_core::TextStyle) -> &'static str {
    match style {
        acorde_core::TextStyle::ChordSymbol => "ChordSymbol",
        acorde_core::TextStyle::RehearsalMark => "RehearsalMark",
        acorde_core::TextStyle::Technique => "Technique",
        acorde_core::TextStyle::Expression => "Expression",
        acorde_core::TextStyle::Lyrics => "Lyrics",
        acorde_core::TextStyle::FiguredBass => "FiguredBass",
        acorde_core::TextStyle::Generic => "Expression",
    }
}

fn pitch_tpc(pitch: &Pitch) -> i32 {
    let natural = match pitch.step {
        Step::F => 13,
        Step::C => 14,
        Step::G => 15,
        Step::D => 16,
        Step::A => 17,
        Step::E => 18,
        Step::B => 19,
    };
    natural + i32::from(pitch.alter) * 7
}

fn microtone_subtype(cents: i16) -> Option<&'static str> {
    match cents {
        50 => Some("quarter-sharp"),
        -50 => Some("quarter-flat"),
        _ => None,
    }
}

fn articulation_subtype(articulation: &Articulation) -> Option<&'static str> {
    match articulation {
        Articulation::Staccato => Some("articStaccatoAbove"),
        Articulation::Staccatissimo => Some("articStaccatissimoAbove"),
        Articulation::Accent => Some("articAccentAbove"),
        Articulation::Tenuto => Some("articTenutoAbove"),
        Articulation::Marcato => Some("articMarcatoAbove"),
        Articulation::Tremolo(_)
        | Articulation::Fermata
        | Articulation::Trill
        | Articulation::Mordent
        | Articulation::InvertedMordent
        | Articulation::Turn
        | Articulation::InvertedTurn
        | Articulation::Shake
        | Articulation::BreathMark
        | Articulation::Caesura => None,
    }
}

fn note_head_name(note_head: &NoteHead) -> &'static str {
    match note_head {
        NoteHead::Normal => "normal",
        NoteHead::Diamond => "diamond",
        NoteHead::X => "x",
        NoteHead::Slash => "slash",
        NoteHead::Cross => "cross",
        NoteHead::Triangle => "triangle",
    }
}

fn beam_mode(beam: BeamState) -> Option<&'static str> {
    match beam {
        BeamState::None => None,
        BeamState::Begin => Some("begin"),
        BeamState::Continue => Some("continue"),
        BeamState::End => Some("end"),
        BeamState::BeginEnd | BeamState::BackwardHook | BeamState::ForwardHook => None,
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn fmt_error(_: std::fmt::Error) -> Error {
    Error::Xml("MSCX serialization formatting failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Barline, Part, Score};

    #[test]
    fn mscx_and_mscz_round_trip_the_canonical_subset() {
        let mut score = Score::default();
        score.parts.clear();
        score.metadata.title = "Title".to_string();
        score.metadata.composer = "Composer".to_string();
        score.metadata.lyricist = "Lyricist".to_string();
        score.metadata.copyright = "Copyright".to_string();
        score.metadata.work_number = "Op. 1".to_string();
        score.metadata.movement_title = "Movement".to_string();
        let mut part = Part::new("Export & Test", "ET");
        part.midi_channel = 7;
        let mut staff = Staff::new(Clef::Treble);
        staff.measures.push(acorde_core::Measure::empty(4, 4));
        staff.measures[0].barline_right = Barline::RepeatEnd;
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.stem_up = Some(false);
        note.note_head = NoteHead::Diamond;
        note.beam = BeamState::Begin;
        note.tie_start = true;
        note.tie_end = true;
        staff.measures[0].voices[0][0] = note;
        part.staves.push(staff);
        score.parts.push(part);

        let mscx = serialize_mscx(&score).expect("MSCX serializes");
        let restored = super::super::parse_mscx(&mscx).expect("MSCX reparses");
        assert_eq!(restored.parts[0].name, "Export & Test");
        assert_eq!(restored.parts[0].short_name, "ET");
        assert_eq!(restored.parts[0].midi_channel, 7);
        assert_eq!(restored.metadata.title, score.metadata.title);
        assert_eq!(restored.metadata.composer, score.metadata.composer);
        assert_eq!(restored.metadata.lyricist, score.metadata.lyricist);
        assert_eq!(restored.metadata.copyright, score.metadata.copyright);
        assert_eq!(restored.metadata.work_number, score.metadata.work_number);
        assert_eq!(
            restored.metadata.movement_title,
            score.metadata.movement_title
        );
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].stem_up,
            Some(false)
        );
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].note_head,
            NoteHead::Diamond
        );
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].beam,
            BeamState::Begin
        );
        assert!(restored.parts[0].staves[0].measures[0].voices[0][0].tie_start);
        assert!(restored.parts[0].staves[0].measures[0].voices[0][0].tie_end);
        assert_eq!(restored.parts[0].staves[0].measures[0].voices[0].len(), 1);
        let mscz = serialize_mscz(&score).expect("MSCZ serializes");
        let restored_archive = super::super::parse_mscz(&mscz).expect("MSCZ reparses");
        assert_eq!(restored_archive.parts[0].name, "Export & Test");
    }

    #[test]
    fn supported_quarter_tone_accidentals_round_trip_without_export_loss() {
        let mut score = Score::default();
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.pitches[0].microtone_cents = 50;
        score.parts[0].staves[0].measures[0].voices[0].clear();
        score.parts[0].staves[0].measures[0].voices[0].push(note);
        let report = crate::serialize_mscx_with_report(&score).expect("report serializes");
        assert!(!report.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/note/1"))
        }));
        let restored = super::super::parse_mscx(&report.output).expect("MSCX reparses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].pitches[0].microtone_cents,
            50
        );
    }

    #[test]
    fn offset_staff_text_round_trips_through_mscx() {
        let mut score = Score::default();
        let styled = acorde_core::StyledText {
            style: acorde_core::TextStyle::Expression,
            text: "Chorus".to_string(),
            placement: None,
            offset_x: Some(-1.5),
            offset_y: Some(2.25),
            relative_x: None,
            relative_y: None,
        };
        score.parts[0].staves[0].measures[0]
            .texts
            .push(styled.clone());

        let output = serialize_mscx(&score).expect("MSCX serializes");
        let restored = super::super::parse_mscx(&output).expect("MSCX reparses");
        assert_eq!(restored.parts[0].staves[0].measures[0].texts, vec![styled]);
    }

    #[test]
    fn measure_text_styles_round_trip_through_mscx() {
        let mut score = Score::default();
        let texts = vec![
            acorde_core::StyledText {
                style: acorde_core::TextStyle::Lyrics,
                text: "la".to_string(),
                placement: None,
                offset_x: None,
                offset_y: None,
                relative_x: None,
                relative_y: None,
            },
            acorde_core::StyledText {
                style: acorde_core::TextStyle::FiguredBass,
                text: "6".to_string(),
                placement: None,
                offset_x: None,
                offset_y: None,
                relative_x: None,
                relative_y: None,
            },
        ];
        score.parts[0].staves[0].measures[0].texts = texts.clone();

        let output = serialize_mscx(&score).expect("MSCX serializes");
        let restored = super::super::parse_mscx(&output).expect("MSCX reparses");
        assert_eq!(restored.parts[0].staves[0].measures[0].texts, texts);
    }

    #[test]
    fn score_level_text_round_trips_through_mscx_vbox() {
        let score = Score {
            texts: vec![acorde_core::StyledText {
                style: acorde_core::TextStyle::Expression,
                text: "Title".to_string(),
                placement: None,
                offset_x: None,
                offset_y: None,
                relative_x: None,
                relative_y: None,
            }],
            ..Score::default()
        };

        let output = serialize_mscx(&score).expect("MSCX serializes");
        let restored = super::super::parse_mscx(&output).expect("MSCX reparses");
        assert_eq!(restored.texts, score.texts);
    }

    #[test]
    fn dynamics_round_trip_as_musescore_measure_events() {
        let mut score = Score::default();
        score.parts[0].staves[0].measures[0].voices[0].clear();
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.dynamic = Some(acorde_core::Dynamic::Mf);
        score.parts[0].staves[0].measures[0].voices[0].push(note);
        let report = crate::serialize_mscx_with_report(&score).expect("report serializes");
        assert!(!report.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .loss_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("dynamic"))
        }));
        let restored = super::super::parse_mscx(&report.output).expect("MSCX reparses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].dynamic,
            Some(acorde_core::Dynamic::Mf)
        );
    }

    #[test]
    fn common_articulations_round_trip_as_musescore_chord_elements() {
        let mut score = Score::default();
        score.parts[0].staves[0].measures[0].voices[0].clear();
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations = vec![Articulation::Staccato, Articulation::Accent];
        score.parts[0].staves[0].measures[0].voices[0].push(note);
        let report = crate::serialize_mscx_with_report(&score).expect("report serializes");
        assert!(!report.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .loss_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("articulation"))
        }));
        let restored = super::super::parse_mscx(&report.output).expect("MSCX reparses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].articulations,
            vec![Articulation::Staccato, Articulation::Accent]
        );
    }

    #[test]
    fn export_loss_diagnostics_locate_fields_outside_the_canonical_subset() {
        let mut score = Score::default();
        score.metadata.composer = "Composer".to_string();
        score.parts[0].short_name = "Pno.".to_string();
        score.parts[0].staves[0].transpose_semitones = 12;
        score.parts[0].staves[0].measures[0].rehearsal = Some("A".to_string());
        let report = crate::serialize_mscx_with_report(&score).expect("report serializes");
        assert!(!report.diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref() == Some("/score/metadata/composer")
        }));
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_location.as_deref()
                    == Some("/score/part/1/staff/1/transpose-semitones"))
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_location.as_deref()
                    == Some("/score/part/1/staff/1/measure/1/rehearsal"))
        );
    }
}
