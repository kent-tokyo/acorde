use crate::Error;
use acorde_core::{
    Articulation, Barline, Duration, GuitarTechnique, HairpinKind, HarpPedalPosition,
    NotationSpanner, NotationSpannerKind, Note, NoteAddr, NoteHead, PartGroup, PartGroupSymbol,
    Score, TextStyle, TimeSignature,
};

const DIVISIONS: u32 = 480;

pub fn serialize_musicxml(score: &Score) -> Result<String, Error> {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<score-partwise version=\"4.0\">\n");

    xml.push_str("  <work>\n");
    xml.push_str(&format!(
        "    <work-title>{}</work-title>\n",
        escape_xml(&score.metadata.title)
    ));
    xml.push_str("  </work>\n");

    if !score.metadata.composer.is_empty() {
        xml.push_str("  <identification>\n");
        xml.push_str(&format!(
            "    <creator type=\"composer\">{}</creator>\n",
            escape_xml(&score.metadata.composer)
        ));
        xml.push_str("  </identification>\n");
    }

    xml.push_str("  <part-list>\n");
    // Assign a stable 1-based number to each group in declaration order.
    let numbered_groups: Vec<(&PartGroup, u32)> = score
        .part_groups
        .iter()
        .enumerate()
        .map(|(i, g)| (g, (i + 1) as u32))
        .collect();

    for (pi, part) in score.parts.iter().enumerate() {
        // Emit group starts before this part index.
        for (g, n) in &numbered_groups {
            if g.first_part == pi {
                let sym = match g.symbol {
                    PartGroupSymbol::Brace => "brace",
                    PartGroupSymbol::Line => "line",
                    PartGroupSymbol::Bracket => "bracket",
                };
                xml.push_str(&format!("    <part-group type=\"start\" number=\"{n}\">\n"));
                xml.push_str(&format!("      <group-symbol>{sym}</group-symbol>\n"));
                if g.barlines_connect {
                    xml.push_str("      <group-barline>yes</group-barline>\n");
                }
                xml.push_str("    </part-group>\n");
            }
        }
        xml.push_str(&format!(
            "    <score-part id=\"{}\">\n",
            escape_xml(&part.id)
        ));
        xml.push_str(&format!(
            "      <part-name>{}</part-name>\n",
            escape_xml(&part.name)
        ));
        for instrument in &part.percussion_instruments {
            xml.push_str(&format!(
                "      <score-instrument id=\"{}\">\n",
                escape_xml(&instrument.id)
            ));
            if let Some(name) = &instrument.name {
                xml.push_str(&format!(
                    "        <instrument-name>{}</instrument-name>\n",
                    escape_xml(name)
                ));
            }
            if let Some(key) = instrument.midi_unpitched {
                xml.push_str(&format!("        <midi-unpitched>{key}</midi-unpitched>\n"));
            }
            xml.push_str("      </score-instrument>\n");
        }
        xml.push_str(&format!(
            "      <midi-instrument id=\"{}-I1\">\n",
            escape_xml(&part.id)
        ));
        xml.push_str(&format!(
            "        <midi-channel>{}</midi-channel>\n",
            part.midi_channel + 1
        ));
        xml.push_str(&format!(
            "        <midi-program>{}</midi-program>\n",
            part.midi_program + 1
        ));
        xml.push_str("      </midi-instrument>\n");
        xml.push_str("    </score-part>\n");
        // Emit group stops after this part index.
        for (g, n) in &numbered_groups {
            if g.last_part == pi {
                xml.push_str(&format!("    <part-group type=\"stop\" number=\"{n}\"/>\n"));
            }
        }
    }
    xml.push_str("  </part-list>\n");

    for (pi, part) in score.parts.iter().enumerate() {
        xml.push_str(&format!("  <part id=\"{}\">\n", escape_xml(&part.id)));
        let staff = match part.staves.first() {
            Some(s) => s,
            None => {
                xml.push_str("  </part>\n");
                continue;
            }
        };

        let mut skip_until = 0usize;
        for (i, measure) in staff.measures.iter().enumerate() {
            if i > 0 && i < skip_until {
                continue;
            }

            xml.push_str(&format!("    <measure number=\"{}\">\n", measure.number));
            let measure_time = measure
                .time_sig
                .as_ref()
                .unwrap_or(&score.settings.time_signature);
            let measure_ticks = time_signature_ticks(measure_time).unwrap_or(DIVISIONS * 4);

            // System / page break (written before barlines, as a print element)
            if measure.system_break || measure.page_break {
                let ns = if measure.system_break {
                    " new-system=\"yes\""
                } else {
                    ""
                };
                let np = if measure.page_break {
                    " new-page=\"yes\""
                } else {
                    ""
                };
                xml.push_str(&format!("      <print{}{}/>\n", ns, np));
            }
            if measure.section_break {
                xml.push_str("      <direction><direction-type><other-direction>acorde:section-break</other-direction></direction-type></direction>\n");
            }

            // Left barline
            let has_left = !matches!(measure.barline_left, Barline::Normal)
                || measure
                    .volta
                    .as_ref()
                    .map(|v| v.kind == "begin" || v.kind == "begin_end")
                    .unwrap_or(false);
            if has_left {
                xml.push_str("      <barline location=\"left\">\n");
                if matches!(measure.barline_left, Barline::RepeatStart) {
                    xml.push_str("        <bar-style>heavy-light</bar-style>\n");
                    xml.push_str("        <repeat direction=\"forward\"/>\n");
                }
                if let Some(v) = &measure.volta {
                    xml.push_str(&format!(
                        "        <ending number=\"{}\" type=\"start\"/>\n",
                        v.number
                    ));
                }
                xml.push_str("      </barline>\n");
            }

            // Attributes. MusicXML attributes persist until changed, so emit the
            // score defaults in the first measure and measure-local overrides
            // wherever they occur. Omitting a later time signature makes a
            // valid score serialize with the wrong cursor length.
            if i == 0
                || measure.key_sig.is_some()
                || measure.time_sig.is_some()
                || measure.clef.is_some()
                || measure.tablature_change.is_some()
            {
                xml.push_str("      <attributes>\n");
                if i == 0 {
                    xml.push_str(&format!("        <divisions>{}</divisions>\n", DIVISIONS));
                }
                if i == 0 || measure.key_sig.is_some() {
                    let key = measure
                        .key_sig
                        .as_ref()
                        .unwrap_or(&score.settings.key_signature);
                    xml.push_str("        <key>\n");
                    xml.push_str(&format!("          <fifths>{}</fifths>\n", key.fifths));
                    xml.push_str(&format!("          <mode>{}</mode>\n", key.mode));
                    xml.push_str("        </key>\n");
                }
                if i == 0 || measure.time_sig.is_some() {
                    let ts = measure
                        .time_sig
                        .as_ref()
                        .unwrap_or(&score.settings.time_signature);
                    xml.push_str("        <time>\n");
                    xml.push_str(&format!("          <beats>{}</beats>\n", ts.numerator));
                    xml.push_str(&format!(
                        "          <beat-type>{}</beat-type>\n",
                        ts.denominator
                    ));
                    xml.push_str("        </time>\n");
                }
                if i == 0 || measure.clef.is_some() {
                    let clef = measure.clef.as_ref().unwrap_or(&staff.clef);
                    xml.push_str("        <clef>\n");
                    xml.push_str(&format!(
                        "          <sign>{}</sign>\n",
                        clef.to_musicxml_sign()
                    ));
                    xml.push_str(&format!(
                        "          <line>{}</line>\n",
                        clef.musicxml_line()
                    ));
                    xml.push_str("        </clef>\n");
                }
                if i == 0 {
                    for (staff_number, extra_staff) in part.staves.iter().enumerate().skip(1) {
                        let has_content = extra_staff
                            .measures
                            .iter()
                            .flat_map(|measure| measure.voices.iter())
                            .any(|voice| !voice.is_empty());
                        if !has_content {
                            continue;
                        }
                        xml.push_str(&format!(
                        "        <clef number=\"{}\">\n          <sign>{}</sign>\n          <line>{}</line>\n        </clef>\n",
                        staff_number + 1,
                        extra_staff.clef.to_musicxml_sign(),
                        extra_staff.clef.musicxml_line()
                    ));
                        if extra_staff.tablature.is_some() || extra_staff.presentation.lines != 5 {
                            xml.push_str(&format!(
                                "        <staff-details number=\"{}\">\n          <staff-lines>{}</staff-lines>\n",
                                staff_number + 1,
                                extra_staff
                                    .tablature
                                    .as_ref()
                                    .map_or(extra_staff.presentation.lines, |tab| tab.lines),
                            ));
                            if let Some(tab) = &extra_staff.tablature {
                                for (index, &midi) in tab.tuning_midi.iter().enumerate() {
                                    if index >= usize::from(tab.lines) {
                                        break;
                                    }
                                    let (step, alter, octave) = midi_to_musicxml_tuning(midi);
                                    xml.push_str(&format!(
                                        "          <staff-tuning line=\"{}\"><tuning-step>{}</tuning-step><tuning-alter>{}</tuning-alter><tuning-octave>{}</tuning-octave></staff-tuning>\n",
                                        index + 1,
                                        step,
                                        alter,
                                        octave
                                    ));
                                }
                            }
                            xml.push_str("        </staff-details>\n");
                        }
                    }
                }
                let tablature = if let Some(change) = measure.tablature_change.as_ref() {
                    Some(change)
                } else if i == 0 {
                    staff.tablature.as_ref()
                } else {
                    None
                };
                if let Some(tab) = tablature {
                    xml.push_str("        <staff-details>\n");
                    xml.push_str(&format!(
                        "          <staff-lines>{}</staff-lines>\n",
                        tab.lines
                    ));
                    for (index, &midi) in tab.tuning_midi.iter().enumerate() {
                        if index >= usize::from(tab.lines) {
                            break;
                        }
                        let (step, alter, octave) = midi_to_musicxml_tuning(midi);
                        xml.push_str(&format!(
                            "          <staff-tuning line=\"{}\"><tuning-step>{}</tuning-step><tuning-alter>{}</tuning-alter><tuning-octave>{}</tuning-octave></staff-tuning>\n",
                            index + 1,
                            step,
                            alter,
                            octave
                        ));
                    }
                    xml.push_str("        </staff-details>\n");
                } else if i == 0 && staff.presentation.lines != 5 {
                    xml.push_str("        <staff-details>\n");
                    xml.push_str(&format!(
                        "          <staff-lines>{}</staff-lines>\n",
                        staff.presentation.lines
                    ));
                    xml.push_str("        </staff-details>\n");
                }
                if i == 0 && staff.transpose_semitones != 0 {
                    xml.push_str("        <transpose>\n");
                    xml.push_str(&format!(
                        "          <chromatic>{}</chromatic>\n",
                        staff.transpose_semitones
                    ));
                    xml.push_str("        </transpose>\n");
                }
                if let Some(count) = measure.multi_rest_count
                    && count >= 2
                {
                    xml.push_str("        <measure-style>\n");
                    xml.push_str(&format!(
                        "          <multiple-rest>{}</multiple-rest>\n",
                        count
                    ));
                    xml.push_str("        </measure-style>\n");
                    skip_until = i + count as usize;
                }
                xml.push_str("      </attributes>\n");
                xml.push_str("      <direction placement=\"above\">\n");
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!(
                    "          <metronome><beat-unit>quarter</beat-unit><per-minute>{}</per-minute></metronome>\n",
                    score.settings.tempo_bpm
                ));
                xml.push_str("        </direction-type>\n");
                xml.push_str(&format!(
                    "        <sound tempo=\"{}\"/>\n",
                    score.settings.tempo_bpm
                ));
                xml.push_str("      </direction>\n");
            } else if let Some(count) = measure.multi_rest_count
                && count >= 2
            {
                xml.push_str("      <attributes>\n");
                xml.push_str("        <measure-style>\n");
                xml.push_str(&format!(
                    "          <multiple-rest>{}</multiple-rest>\n",
                    count
                ));
                xml.push_str("        </measure-style>\n");
                xml.push_str("      </attributes>\n");
                skip_until = i + count as usize;
            }

            // Per-measure tempo change (measures after the first)
            if i > 0
                && let Some(bpm) = measure.tempo
            {
                xml.push_str("      <direction placement=\"above\">\n");
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!(
                        "          <metronome><beat-unit>quarter</beat-unit><per-minute>{}</per-minute></metronome>\n",
                        bpm
                    ));
                xml.push_str("        </direction-type>\n");
                xml.push_str(&format!("        <sound tempo=\"{}\"/>\n", bpm));
                xml.push_str("      </direction>\n");
            }

            // Navigation mark
            if let Some(nav) = &measure.navigation {
                let (dtype_xml, sound_attr) = navigation_direction(nav);
                xml.push_str("      <direction placement=\"above\">\n");
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!("          {}\n", dtype_xml));
                xml.push_str("        </direction-type>\n");
                if let Some(attr) = sound_attr {
                    xml.push_str(&format!("        <sound {}=\"yes\"/>\n", attr));
                }
                xml.push_str("      </direction>\n");
            }

            // Rehearsal mark
            if let Some(reh) = &measure.rehearsal {
                let attrs = measure_text_direction_attrs(measure, TextStyle::RehearsalMark, reh);
                xml.push_str(&format!("      <direction{}>\n", attrs));
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!(
                    "          <rehearsal>{}</rehearsal>\n",
                    escape_xml(reh)
                ));
                xml.push_str("        </direction-type>\n");
                xml.push_str("      </direction>\n");
            }

            // Tempo text
            if let Some(text) = &measure.tempo_text {
                let attrs = measure_text_direction_attrs(measure, TextStyle::Generic, text);
                xml.push_str(&format!("      <direction{}>\n", attrs));
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!("          <words>{}</words>\n", escape_xml(text)));
                xml.push_str("        </direction-type>\n");
                xml.push_str("      </direction>\n");
            }

            // Expression text (dolce, espressivo, etc. — no <sound> element)
            if let Some(text) = &measure.expression_text {
                let attrs = measure_text_direction_attrs(measure, TextStyle::Expression, text);
                xml.push_str(&format!("      <direction{}>\n", attrs));
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!("          <words>{}</words>\n", escape_xml(text)));
                xml.push_str("        </direction-type>\n");
                xml.push_str("      </direction>\n");
            }

            for diagram in &measure.harp_pedal_diagrams {
                let placement = diagram
                    .placement
                    .as_deref()
                    .filter(|placement| matches!(*placement, "above" | "below"))
                    .map(|placement| format!(" placement=\"{placement}\""))
                    .unwrap_or_default();
                xml.push_str(&format!("      <direction{placement}>\n"));
                xml.push_str("        <direction-type>\n          <harp-pedals>\n");
                for (step, position) in ["D", "C", "B", "E", "F", "G", "A"]
                    .iter()
                    .zip(diagram.positions.iter())
                {
                    let alter = match position {
                        HarpPedalPosition::Flat => -1,
                        HarpPedalPosition::Natural => 0,
                        HarpPedalPosition::Sharp => 1,
                    };
                    xml.push_str(&format!(
                        "            <pedal-tuning><pedal-step>{step}</pedal-step><pedal-alter>{alter}</pedal-alter></pedal-tuning>\n"
                    ));
                }
                xml.push_str(
                    "          </harp-pedals>\n        </direction-type>\n      </direction>\n",
                );
            }

            if !measure.figured_bass.is_empty() {
                xml.push_str("      <figured-bass>\n");
                for figure in &measure.figured_bass {
                    xml.push_str("        <figure>\n");
                    if let Some(prefix) = &figure.prefix {
                        xml.push_str(&format!(
                            "          <prefix>{}</prefix>\n",
                            escape_xml(prefix)
                        ));
                    }
                    xml.push_str(&format!(
                        "          <figure-number>{}</figure-number>\n",
                        escape_xml(&figure.number)
                    ));
                    if let Some(alter) = &figure.alter {
                        xml.push_str(&format!(
                            "          <figure-alter>{}</figure-alter>\n",
                            escape_xml(alter)
                        ));
                    }
                    if let Some(suffix) = &figure.suffix {
                        xml.push_str(&format!(
                            "          <suffix>{}</suffix>\n",
                            escape_xml(suffix)
                        ));
                    }
                    xml.push_str("        </figure>\n");
                }
                xml.push_str("      </figured-bass>\n");
            }

            for styled in &measure.texts {
                let already_emitted = matches!(styled.style, TextStyle::Expression)
                    && measure.expression_text.as_deref() == Some(styled.text.as_str())
                    || matches!(styled.style, TextStyle::RehearsalMark)
                        && measure.rehearsal.as_deref() == Some(styled.text.as_str());
                if already_emitted {
                    continue;
                }
                if styled.style == TextStyle::FiguredBass {
                    if measure.figured_bass.is_empty() {
                        xml.push_str("      <figured-bass><figure><figure-number>");
                        xml.push_str(&escape_xml(&styled.text));
                        xml.push_str("</figure-number></figure></figured-bass>\n");
                    }
                    continue;
                }
                if styled.style == TextStyle::RehearsalMark {
                    let attrs = styled_direction_attrs(styled);
                    xml.push_str(&format!(
                        "      <direction{}><direction-type><rehearsal>",
                        attrs
                    ));
                    xml.push_str(&escape_xml(&styled.text));
                    xml.push_str("</rehearsal></direction-type></direction>\n");
                    continue;
                }
                let attrs = styled_direction_attrs(styled);
                xml.push_str(&format!(
                    "      <direction{}><direction-type><words>",
                    attrs
                ));
                xml.push_str(&escape_xml(&styled.text));
                xml.push_str("</words></direction-type></direction>\n");
            }

            // Emit each populated voice in stable model order. MusicXML advances one shared
            // measure cursor, so return to the measure start before every subsequent voice.
            let mut emitted_voice = false;
            let mut cursor_ticks = 0u32;
            for (voice_index, voice) in measure.voices.iter().enumerate() {
                if voice.is_empty() {
                    continue;
                }
                if emitted_voice {
                    xml.push_str("      <backup>\n");
                    xml.push_str(&format!("        <duration>{}</duration>\n", cursor_ticks));
                    xml.push_str("      </backup>\n");
                }
                for (note_index, note) in voice.iter().enumerate() {
                    serialize_note(
                        &mut xml,
                        note,
                        musicxml_voice_number(measure, voice_index),
                        1,
                        measure_ticks,
                        score,
                        &NoteAddr {
                            part: pi,
                            staff: 0,
                            measure: i,
                            voice: voice_index,
                            note: note_index,
                        },
                    );
                }
                cursor_ticks = serialized_voice_ticks(voice, measure_ticks);
                emitted_voice = true;
            }

            // A part may contain multiple staves (for example, a piano grand
            // staff). Return to the measure start before each additional staff
            // and retain its ownership through the per-note MusicXML staff tag.
            for (staff_index, extra_staff) in part.staves.iter().enumerate().skip(1) {
                let Some(extra_measure) = extra_staff.measures.get(i) else {
                    continue;
                };
                if emitted_voice {
                    xml.push_str("      <backup>\n");
                    xml.push_str(&format!("        <duration>{}</duration>\n", cursor_ticks));
                    xml.push_str("      </backup>\n");
                }
                let mut extra_emitted_voice = false;
                for (voice_index, voice) in extra_measure.voices.iter().enumerate() {
                    if voice.is_empty() {
                        continue;
                    }
                    if extra_emitted_voice {
                        xml.push_str("      <backup>\n");
                        xml.push_str(&format!("        <duration>{}</duration>\n", cursor_ticks));
                        xml.push_str("      </backup>\n");
                    }
                    for (note_index, note) in voice.iter().enumerate() {
                        serialize_note(
                            &mut xml,
                            note,
                            musicxml_voice_number(extra_measure, voice_index),
                            staff_index + 1,
                            measure_ticks,
                            score,
                            &NoteAddr {
                                part: pi,
                                staff: staff_index,
                                measure: i,
                                voice: voice_index,
                                note: note_index,
                            },
                        );
                    }
                    cursor_ticks = serialized_voice_ticks(voice, measure_ticks);
                    extra_emitted_voice = true;
                    emitted_voice = true;
                }
            }

            // Right barline
            let has_right = !matches!(measure.barline_right, Barline::Normal)
                || measure
                    .volta
                    .as_ref()
                    .map(|v| v.kind == "end" || v.kind == "begin_end")
                    .unwrap_or(false);
            if has_right {
                xml.push_str("      <barline location=\"right\">\n");
                match measure.barline_right {
                    Barline::RepeatEnd => {
                        xml.push_str("        <bar-style>light-heavy</bar-style>\n");
                        xml.push_str("        <repeat direction=\"backward\"/>\n");
                    }
                    Barline::Final => xml.push_str("        <bar-style>light-heavy</bar-style>\n"),
                    Barline::Double => xml.push_str("        <bar-style>light-light</bar-style>\n"),
                    _ => {}
                }
                if let Some(v) = &measure.volta
                    && (v.kind == "end" || v.kind == "begin_end")
                {
                    xml.push_str(&format!(
                        "        <ending number=\"{}\" type=\"stop\"/>\n",
                        v.number
                    ));
                }
                xml.push_str("      </barline>\n");
            }

            xml.push_str("    </measure>\n");
        }
        xml.push_str("  </part>\n");
    }
    xml.push_str("</score-partwise>\n");
    Ok(xml)
}

fn serialize_note(
    xml: &mut String,
    note: &Note,
    voice_number: u32,
    staff_number: usize,
    measure_ticks: u32,
    score: &Score,
    address: &NoteAddr,
) {
    let tab_lines = score
        .parts
        .get(address.part)
        .and_then(|part| part.staves.get(address.staff))
        .and_then(|staff| staff.tablature.as_ref())
        .map(|tab| tab.lines);
    let musicxml_tab_string = |string: u8| {
        score
            .parts
            .get(address.part)
            .and_then(|part| part.staves.get(address.staff))
            .and_then(|staff| staff.tablature.as_ref())
            .and_then(|tab| (string != 0 && string <= tab.lines).then_some(tab.lines + 1 - string))
            .unwrap_or(string)
    };
    let typed_spanners: Vec<&NotationSpanner> = score
        .spanners
        .iter()
        .filter(|spanner| spanner.start == *address || spanner.end == *address)
        .collect();
    serialize_typed_direction_spanners(xml, &typed_spanners, address);
    let has_typed_endpoint =
        |kind: NotationSpannerKind| typed_spanners.iter().any(|spanner| spanner.kind == kind);

    // Pedal start
    if note.pedal_start && !has_typed_endpoint(NotationSpannerKind::Pedal) {
        xml.push_str("      <direction placement=\"below\">\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str("          <pedal type=\"start\" line=\"no\"/>\n");
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }

    // Ottava start
    if let Some(ok) = &note.ottava_start
        && !has_typed_endpoint(NotationSpannerKind::Ottava)
    {
        xml.push_str("      <direction placement=\"above\">\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str(&format!(
            "          <octave-shift type=\"{}\" size=\"{}\" number=\"1\"/>\n",
            ok.musicxml_type(),
            ok.musicxml_size()
        ));
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }

    // Chord symbol
    if let Some(cs) = &note.chord_symbol {
        let (root_step, root_alter) = split_note_name(&cs.root);
        xml.push_str("      <harmony");
        if let Some(placement) = &cs.placement {
            xml.push_str(&format!(" placement=\"{}\"", escape_xml(placement)));
        }
        xml.push_str(">\n");
        xml.push_str("        <root>\n");
        xml.push_str(&format!(
            "          <root-step>{}</root-step>\n",
            escape_xml(root_step)
        ));
        if root_alter != 0 {
            xml.push_str(&format!(
                "          <root-alter>{}</root-alter>\n",
                root_alter
            ));
        }
        xml.push_str("        </root>\n");
        xml.push_str(&format!("        <kind>{}</kind>\n", escape_xml(&cs.kind)));
        if let Some(function) = &cs.harmony_function {
            xml.push_str(&format!(
                "        <function>{}</function>\n",
                escape_xml(function)
            ));
        }
        if let Some(bass) = &cs.bass {
            let (bass_step, bass_alter) = split_note_name(bass);
            xml.push_str("        <bass>\n");
            xml.push_str(&format!(
                "          <bass-step>{}</bass-step>\n",
                escape_xml(bass_step)
            ));
            if bass_alter != 0 {
                xml.push_str(&format!(
                    "          <bass-alter>{}</bass-alter>\n",
                    bass_alter
                ));
            }
            xml.push_str("        </bass>\n");
        }
        for degree in &cs.degrees {
            xml.push_str("        <degree>\n");
            xml.push_str(&format!(
                "          <degree-value>{}</degree-value>\n",
                degree.value
            ));
            if degree.alter != 0 {
                xml.push_str(&format!(
                    "          <degree-alter>{}</degree-alter>\n",
                    degree.alter
                ));
            }
            if !degree.kind.is_empty() {
                xml.push_str(&format!(
                    "          <degree-type>{}</degree-type>\n",
                    escape_xml(&degree.kind)
                ));
            }
            xml.push_str("        </degree>\n");
        }
        xml.push_str("      </harmony>\n");
    }

    // Dynamic
    if let Some(dyn_val) = &note.dynamic {
        xml.push_str("      <direction placement=\"below\">\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str(&format!(
            "          <dynamics><{}/></dynamics>\n",
            dyn_val.to_musicxml_str()
        ));
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }

    // Hairpin start
    if let Some(hp) = &note.hairpin_start {
        let wedge_type = match hp {
            HairpinKind::Crescendo => "crescendo",
            HairpinKind::Decrescendo => "diminuendo",
        };
        xml.push_str("      <direction placement=\"below\">\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str(&format!("          <wedge type=\"{}\"/>\n", wedge_type));
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }

    let (dur_ticks, duration_type, dot_count, is_measure_rest) =
        serialized_note_timing(note, measure_ticks);

    if note.is_rest {
        xml.push_str(&format!("      <note{}>\n", note_placement_attrs(note)));
        if is_measure_rest {
            xml.push_str("        <rest measure=\"yes\"/>\n");
        } else {
            xml.push_str("        <rest/>\n");
        }
        xml.push_str(&format!("        <duration>{}</duration>\n", dur_ticks));
        if let Some(tuplet) = &note.tuplet {
            xml.push_str("        <time-modification>\n");
            xml.push_str(&format!(
                "          <actual-notes>{}</actual-notes>\n",
                tuplet.actual_notes
            ));
            xml.push_str(&format!(
                "          <normal-notes>{}</normal-notes>\n",
                tuplet.normal_notes
            ));
            xml.push_str("        </time-modification>\n");
        }
        xml.push_str(&format!("        <voice>{voice_number}</voice>\n"));
        if let Some(cross) = &note.cross_staff {
            xml.push_str(&format!(
                "        <staff>{}</staff>\n",
                cross.target_staff + 1
            ));
        } else {
            xml.push_str(&format!("        <staff>{staff_number}</staff>\n"));
        }
        xml.push_str(&format!("        <type>{}</type>\n", duration_type));
        for _ in 0..dot_count {
            xml.push_str("        <dot/>\n");
        }
        xml.push_str("      </note>\n");
    } else if let Some(pitch) = note.pitches.first() {
        xml.push_str(&format!("      <note{}>\n", note_placement_attrs(note)));
        if note.is_grace {
            if note.grace_slash {
                xml.push_str("        <grace slash=\"yes\"/>\n");
            } else {
                xml.push_str("        <grace/>\n");
            }
        }
        if note.is_cue {
            xml.push_str("        <cue/>\n");
        }
        if note.is_unpitched {
            xml.push_str("        <unpitched>\n");
            xml.push_str(&format!(
                "          <display-step>{}</display-step>\n",
                pitch.step.to_char()
            ));
            xml.push_str(&format!(
                "          <display-octave>{}</display-octave>\n",
                pitch.octave
            ));
            xml.push_str("        </unpitched>\n");
        } else {
            xml.push_str("        <pitch>\n");
            xml.push_str(&format!(
                "          <step>{}</step>\n",
                pitch.step.to_char()
            ));
            if pitch.alter != 0 || pitch.microtone_cents != 0 {
                let alter = pitch.alter as f32 + pitch.microtone_cents as f32 / 100.0;
                xml.push_str(&format!("          <alter>{alter}</alter>\n"));
            }
            xml.push_str(&format!("          <octave>{}</octave>\n", pitch.octave));
            xml.push_str("        </pitch>\n");
        }
        if let Some(instrument_id) = &note.instrument_id {
            xml.push_str(&format!(
                "        <instrument id=\"{}\"/>\n",
                escape_xml(instrument_id)
            ));
        }
        if !note.is_grace {
            xml.push_str(&format!("        <duration>{}</duration>\n", dur_ticks));
            if let Some(tuplet) = &note.tuplet {
                xml.push_str("        <time-modification>\n");
                xml.push_str(&format!(
                    "          <actual-notes>{}</actual-notes>\n",
                    tuplet.actual_notes
                ));
                xml.push_str(&format!(
                    "          <normal-notes>{}</normal-notes>\n",
                    tuplet.normal_notes
                ));
                xml.push_str("        </time-modification>\n");
            }
        }
        if note.tie_end {
            xml.push_str("        <tie type=\"stop\"/>\n");
        }
        if note.tie_start {
            xml.push_str("        <tie type=\"start\"/>\n");
        }
        xml.push_str(&format!("        <voice>{voice_number}</voice>\n"));
        if let Some(cross) = &note.cross_staff {
            xml.push_str(&format!(
                "        <staff>{}</staff>\n",
                cross.target_staff + 1
            ));
        } else {
            xml.push_str(&format!("        <staff>{staff_number}</staff>\n"));
        }
        xml.push_str(&format!(
            "        <type>{}</type>\n",
            note.duration.to_musicxml_type()
        ));
        for _ in 0..note.dot_count {
            xml.push_str("        <dot/>\n");
        }
        if let Some(up) = note.stem_up {
            xml.push_str(&format!(
                "        <stem>{}</stem>\n",
                if up { "up" } else { "down" }
            ));
        }
        if note.note_head != NoteHead::Normal {
            let nh_str = match note.note_head {
                NoteHead::Diamond => "diamond",
                NoteHead::X => "x",
                NoteHead::Slash => "slash",
                NoteHead::Cross => "cross",
                NoteHead::Triangle => "triangle",
                NoteHead::Normal => "normal",
            };
            xml.push_str(&format!("        <notehead>{}</notehead>\n", nh_str));
        }
        serialize_notations(xml, note, &typed_spanners, address, tab_lines);
        if let Some(lyric) = &note.lyric {
            xml.push_str("        <lyric number=\"1\">\n");
            xml.push_str(&format!(
                "          <syllabic>{}</syllabic>\n",
                escape_xml(&lyric.syllabic)
            ));
            xml.push_str(&format!(
                "          <text>{}</text>\n",
                escape_xml(&lyric.text)
            ));
            xml.push_str("        </lyric>\n");
        }
        xml.push_str("      </note>\n");

        // Additional chord pitches
        for (pitch_idx, extra) in note.pitches.iter().skip(1).enumerate() {
            xml.push_str(&format!("      <note{}>\n", note_placement_attrs(note)));
            xml.push_str("        <chord/>\n");
            if note.is_unpitched {
                xml.push_str("        <unpitched>\n");
                xml.push_str(&format!(
                    "          <display-step>{}</display-step>\n",
                    extra.step.to_char()
                ));
                xml.push_str(&format!(
                    "          <display-octave>{}</display-octave>\n",
                    extra.octave
                ));
                xml.push_str("        </unpitched>\n");
            } else {
                xml.push_str("        <pitch>\n");
                xml.push_str(&format!(
                    "          <step>{}</step>\n",
                    extra.step.to_char()
                ));
                if extra.alter != 0 || extra.microtone_cents != 0 {
                    let alter = extra.alter as f32 + extra.microtone_cents as f32 / 100.0;
                    xml.push_str(&format!("          <alter>{alter}</alter>\n"));
                }
                xml.push_str(&format!("          <octave>{}</octave>\n", extra.octave));
                xml.push_str("        </pitch>\n");
            }
            if let Some(instrument_id) = &note.instrument_id {
                xml.push_str(&format!(
                    "        <instrument id=\"{}\"/>\n",
                    escape_xml(instrument_id)
                ));
            }
            xml.push_str(&format!("        <duration>{}</duration>\n", dur_ticks));
            xml.push_str(&format!("        <voice>{voice_number}</voice>\n"));
            xml.push_str(&format!("        <staff>{staff_number}</staff>\n"));
            xml.push_str(&format!(
                "        <type>{}</type>\n",
                note.duration.to_musicxml_type()
            ));
            if let Some(tab) = note.tab_positions.get(pitch_idx + 1) {
                xml.push_str("        <technical>\n");
                xml.push_str(&format!(
                    "          <string>{}</string>\n",
                    musicxml_tab_string(tab.string)
                ));
                xml.push_str(&format!("          <fret>{}</fret>\n", tab.fret));
                xml.push_str("        </technical>\n");
            }
            xml.push_str("      </note>\n");
        }
    }

    // Pedal stop
    if note.pedal_end && !has_typed_endpoint(NotationSpannerKind::Pedal) {
        xml.push_str("      <direction placement=\"below\">\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str("          <pedal type=\"stop\" line=\"no\"/>\n");
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }

    // Ottava stop
    if note.ottava_end && !has_typed_endpoint(NotationSpannerKind::Ottava) {
        xml.push_str("      <direction>\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str("          <octave-shift type=\"stop\" number=\"1\"/>\n");
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }

    // Hairpin stop
    if note.hairpin_end {
        xml.push_str("      <direction placement=\"below\">\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str("          <wedge type=\"stop\"/>\n");
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }
}

fn serialized_voice_ticks(voice: &[Note], measure_ticks: u32) -> u32 {
    voice.iter().fold(0u32, |ticks, note| {
        ticks.saturating_add(serialized_note_timing(note, measure_ticks).0)
    })
}

fn musicxml_voice_number(measure: &acorde_core::Measure, slot: usize) -> u32 {
    measure.source_voice_numbers[slot].unwrap_or((slot + 1) as u32)
}

fn serialized_note_timing(note: &Note, measure_ticks: u32) -> (u32, &'static str, u8, bool) {
    let ticks = serialized_note_ticks(note);
    if note.is_rest
        && matches!(note.duration, Duration::Whole)
        && note.dot_count == 0
        && ticks != measure_ticks
        && let Some((duration_type, dot_count)) = duration_notation_for_ticks(measure_ticks)
    {
        return (measure_ticks, duration_type, dot_count, true);
    }
    (
        ticks,
        note.duration.to_musicxml_type(),
        note.dot_count,
        false,
    )
}

fn duration_notation_for_ticks(ticks: u32) -> Option<(&'static str, u8)> {
    const NOTATIONS: &[(u32, &str, u8)] = &[
        (DIVISIONS * 4, "whole", 0),
        (DIVISIONS * 3, "half", 1),
        (DIVISIONS * 2, "half", 0),
        (DIVISIONS * 3 / 2, "quarter", 1),
        (DIVISIONS, "quarter", 0),
        (DIVISIONS * 3 / 4, "eighth", 1),
        (DIVISIONS / 2, "eighth", 0),
        (DIVISIONS * 3 / 8, "16th", 1),
        (DIVISIONS / 4, "16th", 0),
    ];
    NOTATIONS
        .iter()
        .find_map(|(candidate, duration_type, dots)| {
            (*candidate == ticks).then_some((*duration_type, *dots))
        })
}

fn time_signature_ticks(time: &TimeSignature) -> Option<u32> {
    let numerator = u64::from(time.numerator)
        .checked_mul(4)?
        .checked_mul(u64::from(DIVISIONS))?;
    let ticks = numerator.checked_div(u64::from(time.denominator))?;
    u32::try_from(ticks).ok()
}

fn serialized_note_ticks(note: &Note) -> u32 {
    if note.is_grace || note.is_cue {
        return 0;
    }
    let base = u64::from(note.duration.to_ticks(note.dot_count));
    let adjusted = if let Some(tuplet) = &note.tuplet {
        if tuplet.actual_notes == 0 {
            return 0;
        }
        base.saturating_mul(u64::from(tuplet.normal_notes)) / u64::from(tuplet.actual_notes)
    } else {
        base
    };
    adjusted.min(u64::from(u32::MAX)) as u32
}

fn serialize_typed_direction_spanners(
    xml: &mut String,
    spanners: &[&NotationSpanner],
    address: &NoteAddr,
) {
    for spanner in spanners {
        let endpoint = if spanner.start == *address {
            Some("start")
        } else if spanner.end == *address {
            Some("stop")
        } else {
            None
        };
        let Some(endpoint) = endpoint else {
            continue;
        };
        let number = spanner.number.unwrap_or(1);
        match spanner.kind {
            NotationSpannerKind::Pedal => {
                let placement = spanner.placement.as_deref().unwrap_or("below");
                let line = spanner.line_type.as_deref().unwrap_or("no");
                xml.push_str(&format!(
                    "      <direction placement=\"{}\">\n",
                    escape_xml(placement)
                ));
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!(
                    "          <pedal type=\"{endpoint}\" number=\"{number}\" line=\"{}\"/>\n",
                    escape_xml(line)
                ));
                xml.push_str("        </direction-type>\n");
                xml.push_str("      </direction>\n");
            }
            NotationSpannerKind::Ottava => {
                let placement = spanner.placement.as_deref().unwrap_or("above");
                let shift_type = if endpoint == "stop" {
                    "stop"
                } else {
                    spanner.ottava_type.as_deref().unwrap_or("up")
                };
                let size = spanner.ottava_size.unwrap_or(8);
                xml.push_str(&format!(
                    "      <direction placement=\"{}\">\n",
                    escape_xml(placement)
                ));
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!(
                    "          <octave-shift type=\"{}\" size=\"{size}\" number=\"{number}\"/>\n",
                    escape_xml(shift_type)
                ));
                xml.push_str("        </direction-type>\n");
                xml.push_str("      </direction>\n");
            }
            _ => {}
        }
    }
}

fn serialize_notations(
    xml: &mut String,
    note: &Note,
    typed_spanners: &[&NotationSpanner],
    address: &NoteAddr,
    tab_lines: Option<u8>,
) {
    let typed_has =
        |kind: NotationSpannerKind| typed_spanners.iter().any(|spanner| spanner.kind == kind);
    let has_tie = note.tie_start || note.tie_end;
    let has_glissando =
        note.glissando_start || note.glissando_end || typed_has(NotationSpannerKind::Glissando);
    let has_slur = note.slur_start || note.slur_end || typed_has(NotationSpannerKind::Slur);
    let has_trill_line =
        note.trill_line_start || note.trill_line_end || typed_has(NotationSpannerKind::TrillLine);
    let has_artic = !note.articulations.is_empty();
    let has_arp = note.arpeggiate.is_some();
    let has_technical = note.fingering.is_some()
        || !note.fingerings.is_empty()
        || note.string_number.is_some()
        || note.tab_position.is_some()
        || note.technique_text.is_some()
        || note.guitar_technique.is_some();
    if !has_tie
        && !has_glissando
        && !has_slur
        && !has_trill_line
        && !has_artic
        && !has_arp
        && !has_technical
    {
        return;
    }

    xml.push_str("        <notations>\n");
    if note.tie_end {
        xml.push_str("          <tied type=\"stop\"/>\n");
    }
    if note.tie_start {
        xml.push_str("          <tied type=\"start\"/>\n");
    }
    for spanner in typed_spanners {
        let endpoint = if spanner.start == *address {
            Some("start")
        } else if spanner.end == *address {
            Some("stop")
        } else {
            None
        };
        let Some(endpoint) = endpoint else {
            continue;
        };
        let number = spanner.number.unwrap_or(1);
        let line_type = spanner
            .line_type
            .as_ref()
            .map(|line_type| format!(" line-type=\"{}\"", escape_xml(line_type)))
            .unwrap_or_default();
        let placement = spanner
            .placement
            .as_ref()
            .map(|placement| format!(" placement=\"{}\"", escape_xml(placement)))
            .unwrap_or_default();
        match spanner.kind {
            NotationSpannerKind::Slur => xml.push_str(&format!(
                "          <slur number=\"{number}\" type=\"{endpoint}\"{line_type}{placement}/>\n"
            )),
            NotationSpannerKind::Glissando => {
                let text = spanner.text.as_deref().unwrap_or("");
                xml.push_str(&format!(
                    "          <glissando number=\"{number}\" type=\"{endpoint}\"{line_type}{placement}>{}</glissando>\n",
                    escape_xml(text)
                ));
            }
            NotationSpannerKind::TrillLine => xml.push_str(&format!(
                "          <wavy-line number=\"{number}\" type=\"{endpoint}\"{line_type}{placement}/>\n"
            )),
            NotationSpannerKind::Pedal | NotationSpannerKind::Ottava => {}
        }
    }
    if note.slur_end && !typed_has(NotationSpannerKind::Slur) {
        xml.push_str("          <slur number=\"1\" type=\"stop\"/>\n");
    }
    if note.slur_start && !typed_has(NotationSpannerKind::Slur) {
        xml.push_str("          <slur number=\"1\" type=\"start\"/>\n");
    }
    if note.glissando_end && !typed_has(NotationSpannerKind::Glissando) {
        xml.push_str("          <glissando number=\"1\" type=\"stop\"/>\n");
    }
    if note.glissando_start && !typed_has(NotationSpannerKind::Glissando) {
        xml.push_str("          <glissando number=\"1\" type=\"start\">gliss.</glissando>\n");
    }
    if note.trill_line_end && !typed_has(NotationSpannerKind::TrillLine) {
        xml.push_str("          <wavy-line number=\"1\" type=\"stop\"/>\n");
    }
    if note.trill_line_start && !typed_has(NotationSpannerKind::TrillLine) {
        xml.push_str("          <wavy-line number=\"1\" type=\"start\"/>\n");
    }

    if has_artic {
        let mut tags: Vec<&str> = Vec::new();
        let mut fermata = false;
        let mut trill = false;
        let mut mordent = false;
        let mut inverted_mordent = false;
        let mut turn = false;
        let mut inverted_turn = false;
        let mut shake = false;
        let mut tremolo_n: Option<u8> = None;
        let mut breath_mark = false;
        let mut caesura = false;
        for a in &note.articulations {
            match a {
                Articulation::Staccato => tags.push("staccato"),
                Articulation::Staccatissimo => tags.push("staccatissimo"),
                Articulation::Accent => tags.push("accent"),
                Articulation::Tenuto => tags.push("tenuto"),
                Articulation::Marcato => tags.push("strong-accent"),
                Articulation::Fermata => fermata = true,
                Articulation::Trill => trill = true,
                Articulation::Mordent => mordent = true,
                Articulation::InvertedMordent => inverted_mordent = true,
                Articulation::Turn => turn = true,
                Articulation::InvertedTurn => inverted_turn = true,
                Articulation::Shake => shake = true,
                Articulation::Tremolo(n) => tremolo_n = Some(*n),
                Articulation::BreathMark => breath_mark = true,
                Articulation::Caesura => caesura = true,
            }
        }
        if !tags.is_empty() {
            xml.push_str("          <articulations>\n");
            for t in tags {
                xml.push_str(&format!("            <{}/>\n", t));
            }
            xml.push_str("          </articulations>\n");
        }
        let has_ornaments = trill
            || mordent
            || inverted_mordent
            || turn
            || inverted_turn
            || shake
            || tremolo_n.is_some();
        if has_ornaments {
            xml.push_str("          <ornaments>\n");
            if trill {
                xml.push_str("            <trill-mark/>\n");
            }
            if mordent {
                xml.push_str("            <mordent/>\n");
            }
            if inverted_mordent {
                xml.push_str("            <inverted-mordent/>\n");
            }
            if turn {
                xml.push_str("            <turn/>\n");
            }
            if inverted_turn {
                xml.push_str("            <inverted-turn/>\n");
            }
            if shake {
                xml.push_str("            <shake/>\n");
            }
            if let Some(n) = tremolo_n {
                xml.push_str(&format!("            <tremolo>{}</tremolo>\n", n));
            }
            xml.push_str("          </ornaments>\n");
        }
        if fermata {
            xml.push_str("          <fermata/>\n");
        }
        if breath_mark {
            xml.push_str("          <breath-mark/>\n");
        }
        if caesura {
            xml.push_str("          <caesura/>\n");
        }
    }
    if let Some(dir) = note.arpeggiate {
        let dir_str = if dir { "up" } else { "down" };
        xml.push_str(&format!(
            "          <arpeggiate direction=\"{}\"/>\n",
            dir_str
        ));
    }
    if has_technical {
        xml.push_str("          <technical>\n");
        if note.fingerings.is_empty() {
            if let Some(f) = note.fingering {
                xml.push_str(&format!("            <fingering>{}</fingering>\n", f));
            }
        } else {
            for f in &note.fingerings {
                xml.push_str(&format!("            <fingering>{}</fingering>\n", f));
            }
        }
        if let Some(s) = note.string_number {
            xml.push_str(&format!(
                "            <string>{}</string>\n",
                musicxml_tab_string_from_internal(s, tab_lines)
            ));
        }
        if let Some(tab) = &note.tab_position {
            if note.string_number.is_none() {
                xml.push_str(&format!(
                    "            <string>{}</string>\n",
                    musicxml_tab_string_from_internal(tab.string, tab_lines)
                ));
            }
            xml.push_str(&format!("            <fret>{}</fret>\n", tab.fret));
        }
        if let Some(ref t) = note.technique_text {
            xml.push_str(&format!(
                "            <other-technical>{}</other-technical>\n",
                escape_xml(t)
            ));
        }
        if let Some(ref gt) = note.guitar_technique {
            match gt {
                GuitarTechnique::Bend => {
                    let bend_alter = note.guitar_bend_alter_cents.map_or_else(
                        || "0".to_string(),
                        |cents| format!("{:.2}", cents as f32 / 100.0),
                    );
                    xml.push_str(&format!(
                        "            <bend><bend-alter>{bend_alter}</bend-alter></bend>\n"
                    ));
                }
                GuitarTechnique::Slide => {
                    xml.push_str("            <slide type=\"start\" number=\"1\"/>\n")
                }
                GuitarTechnique::HammerOn => xml
                    .push_str("            <hammer-on type=\"start\" number=\"1\">H</hammer-on>\n"),
                GuitarTechnique::PullOff => {
                    xml.push_str("            <pull-off type=\"start\" number=\"1\">P</pull-off>\n")
                }
            }
        }
        xml.push_str("          </technical>\n");
    }
    xml.push_str("        </notations>\n");
}

fn navigation_direction(nav: &str) -> (String, Option<&'static str>) {
    match nav {
        "Segno" => ("<segno/>".into(), Some("segno")),
        "Coda" => ("<coda/>".into(), Some("tocoda")),
        "Fine" => ("<words>Fine</words>".into(), Some("fine")),
        "DaCapo" => ("<words>D.C.</words>".into(), Some("dacapo")),
        "DaCapoAlFine" => ("<words>D.C. al Fine</words>".into(), None),
        "DaCapoAlCoda" => ("<words>D.C. al Coda</words>".into(), None),
        "DalSegno" => ("<words>D.S.</words>".into(), Some("dalsegno")),
        "DalSegnoAlFine" => ("<words>D.S. al Fine</words>".into(), None),
        "DalSegnoAlCoda" => ("<words>D.S. al Coda</words>".into(), None),
        "ToCoda" => ("<words>To Coda</words>".into(), Some("tocoda")),
        other => (format!("<words>{}</words>", escape_xml(other)), None),
    }
}

fn musicxml_tab_string_from_internal(string: u8, tab_lines: Option<u8>) -> u8 {
    match tab_lines {
        Some(lines) if string != 0 && string <= lines => lines + 1 - string,
        _ => string,
    }
}

fn midi_to_musicxml_tuning(midi: i16) -> (&'static str, i8, i8) {
    const STEPS: [(&str, i8); 12] = [
        ("C", 0),
        ("C", 1),
        ("D", 0),
        ("D", 1),
        ("E", 0),
        ("F", 0),
        ("F", 1),
        ("G", 0),
        ("G", 1),
        ("A", 0),
        ("A", 1),
        ("B", 0),
    ];
    let midi = midi.clamp(0, 127);
    let octave = (midi / 12 - 1) as i8;
    let (step, alter) = STEPS[(midi % 12) as usize];
    (step, alter, octave)
}

fn split_note_name(name: &str) -> (&str, i8) {
    if let Some(s) = name.strip_suffix("##") {
        (s, 2)
    } else if let Some(s) = name.strip_suffix('#') {
        (s, 1)
    } else if let Some(s) = name.strip_suffix("bb") {
        (s, -2)
    } else if name.len() > 1 {
        if let Some(s) = name.strip_suffix('b') {
            (s, -1)
        } else {
            (name, 0)
        }
    } else {
        (name, 0)
    }
}

fn measure_text_direction_attrs(
    measure: &acorde_core::Measure,
    style: TextStyle,
    text: &str,
) -> String {
    let styled = measure
        .texts
        .iter()
        .find(|styled| styled.style == style && styled.text == text);
    styled
        .map(styled_direction_attrs)
        .unwrap_or_else(|| " placement=\"above\"".to_string())
}

fn styled_direction_attrs(styled: &acorde_core::StyledText) -> String {
    let mut attrs = format!(
        " placement=\"{}\"",
        escape_xml(styled.placement.as_deref().unwrap_or("above"))
    );
    if let Some(offset_x) = styled.offset_x {
        attrs.push_str(&format!(" default-x=\"{}\"", offset_x));
    }
    if let Some(offset_y) = styled.offset_y {
        attrs.push_str(&format!(" default-y=\"{}\"", offset_y));
    }
    if let Some(relative_x) = styled.relative_x {
        attrs.push_str(&format!(" relative-x=\"{}\"", relative_x));
    }
    if let Some(relative_y) = styled.relative_y {
        attrs.push_str(&format!(" relative-y=\"{}\"", relative_y));
    }
    attrs
}

fn note_placement_attrs(note: &Note) -> String {
    let mut attrs = String::new();
    for (name, value) in [
        ("default-x", note.offset_x),
        ("default-y", note.offset_y),
        ("relative-x", note.relative_x),
        ("relative-y", note.relative_y),
    ] {
        if let Some(value) = value {
            if value.is_finite() {
                attrs.push_str(&format!(" {name}=\"{value}\""));
            }
        }
    }
    attrs
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{
        Command, Duration, NotationSpanner, NotationSpannerKind, Note, NoteAddr, Pitch,
        RemoveSpannerCmd, Score, ScoreEngine, Step, TupletInfo,
    };

    #[test]
    fn serialize_default_score_produces_xml() {
        let score = Score::default();
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<score-partwise"));
        assert!(xml.contains("</score-partwise>"));
        assert!(xml.contains("Untitled Score"));
    }

    #[test]
    fn rest_note_serialized_correctly() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<rest/>"));
    }

    #[test]
    fn typed_numbered_spanners_serialize_without_legacy_duplicates() {
        let mut score = Score::new("Spans", 120, 2, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
        ];
        let start = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let end = NoteAddr {
            note: 1,
            ..start.clone()
        };
        score.spanners = vec![
            NotationSpanner {
                id: "slur-2".to_string(),
                kind: NotationSpannerKind::Slur,
                start: start.clone(),
                end: end.clone(),
                number: Some(2),
                line_type: Some("dashed".to_string()),
                text: None,
                placement: Some("above".to_string()),
                ottava_size: None,
                ottava_type: None,
            },
            NotationSpanner {
                id: "gliss-3".to_string(),
                kind: NotationSpannerKind::Glissando,
                start,
                end,
                number: Some(3),
                line_type: Some("wavy".to_string()),
                text: Some("gliss.".to_string()),
                placement: Some("below".to_string()),
                ottava_size: None,
                ottava_type: None,
            },
            NotationSpanner {
                id: "slur-7".to_string(),
                kind: NotationSpannerKind::Slur,
                start: NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: 0,
                    voice: 0,
                    note: 0,
                },
                end: NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: 0,
                    voice: 0,
                    note: 1,
                },
                number: Some(7),
                line_type: None,
                text: None,
                placement: Some("below".to_string()),
                ottava_size: None,
                ottava_type: None,
            },
            NotationSpanner {
                id: "pedal-4".to_string(),
                kind: NotationSpannerKind::Pedal,
                start: NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: 0,
                    voice: 0,
                    note: 0,
                },
                end: NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: 0,
                    voice: 0,
                    note: 1,
                },
                number: Some(4),
                line_type: Some("yes".to_string()),
                text: None,
                placement: Some("below".to_string()),
                ottava_size: None,
                ottava_type: None,
            },
            NotationSpanner {
                id: "ottava-5".to_string(),
                kind: NotationSpannerKind::Ottava,
                start: NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: 0,
                    voice: 0,
                    note: 0,
                },
                end: NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: 0,
                    voice: 0,
                    note: 1,
                },
                number: Some(5),
                line_type: None,
                text: None,
                placement: Some("above".to_string()),
                ottava_size: Some(15),
                ottava_type: Some("down".to_string()),
            },
        ];

        let xml = serialize_musicxml(&score).expect("serializes typed spanners");
        assert_eq!(xml.matches("<slur number=\"2\"").count(), 2);
        assert_eq!(xml.matches("<slur number=\"7\"").count(), 2);
        assert_eq!(xml.matches("<glissando number=\"3\"").count(), 2);
        assert_eq!(xml.matches("<pedal type=").count(), 2);
        assert!(xml.contains("<octave-shift type=\"down\" size=\"15\" number=\"5\""));
        assert!(!xml.contains("<slur number=\"1\""));
        let reparsed = crate::parse_musicxml(&xml).expect("reparses typed spanners");
        assert_eq!(reparsed.spanners.len(), 5);
        assert!(
            reparsed
                .spanners
                .iter()
                .any(|spanner| spanner.kind == NotationSpannerKind::Slur
                    && spanner.number == Some(2))
        );
        assert!(
            reparsed
                .spanners
                .iter()
                .any(|spanner| spanner.kind == NotationSpannerKind::Slur
                    && spanner.number == Some(7))
        );
        assert!(reparsed.spanners.iter().any(|spanner| {
            spanner.kind == NotationSpannerKind::Glissando && spanner.number == Some(3)
        }));
        assert!(reparsed.spanners.iter().any(|spanner| {
            spanner.kind == NotationSpannerKind::Pedal && spanner.number == Some(4)
        }));
        assert!(reparsed.spanners.iter().any(|spanner| {
            spanner.kind == NotationSpannerKind::Ottava
                && spanner.number == Some(5)
                && spanner.ottava_type.as_deref() == Some("down")
        }));
    }

    #[test]
    fn removing_a_typed_glissando_does_not_resurrect_legacy_endpoints() {
        let mut score = Score::new("Spans", 120, 2, 4, 0, 1);
        let mut start_note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        start_note.glissando_start = true;
        let mut end_note = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        end_note.glissando_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![start_note, end_note];
        let start = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        score.spanners.push(NotationSpanner {
            id: "gliss-1".into(),
            kind: NotationSpannerKind::Glissando,
            start: start.clone(),
            end: NoteAddr { note: 1, ..start },
            number: Some(1),
            line_type: None,
            text: None,
            placement: None,
            ottava_size: None,
            ottava_type: None,
        });
        let mut engine = ScoreEngine::new();
        engine.replace_score(score);
        engine
            .apply(Command::RemoveSpanner(RemoveSpannerCmd {
                id: "gliss-1".into(),
            }))
            .expect("remove typed glissando");
        let xml = serialize_musicxml(&engine.score).expect("serialize after removal");
        assert!(!xml.contains("<glissando"), "{xml}");
    }

    #[test]
    fn typed_spanner_roundtrips_cross_measure_voice_and_staff_endpoints() {
        let mut score = Score::template(acorde_core::ScoreTemplate::Piano);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        score.parts[0].staves[1].measures[1].voices[1] =
            vec![Note::new(Pitch::new(Step::D, 3), Duration::Quarter)];
        score.spanners.push(NotationSpanner {
            id: "cross-staff-gliss".to_string(),
            kind: NotationSpannerKind::Glissando,
            start: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            end: NoteAddr {
                part: 0,
                staff: 1,
                measure: 1,
                voice: 1,
                note: 0,
            },
            number: Some(9),
            line_type: Some("solid".to_string()),
            text: Some("gliss.".to_string()),
            placement: Some("above".to_string()),
            ottava_size: None,
            ottava_type: None,
        });

        let xml = serialize_musicxml(&score).expect("serializes cross-staff span");
        let reparsed = crate::parse_musicxml(&xml).expect("reparses cross-staff span");
        let span = reparsed
            .spanners
            .iter()
            .find(|span| span.number == Some(9))
            .expect("typed span");
        assert_eq!(span.start.measure, 0);
        assert_eq!(span.start.voice, 0);
        assert_eq!(span.start.staff, 0);
        assert_eq!(span.end.measure, 1);
        assert_eq!(span.end.voice, 1);
        assert_eq!(span.end.staff, 1);
    }

    #[test]
    fn irregular_whole_rest_uses_valid_measure_rest_timing() {
        let mut score = Score::new("T", 120, 3, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![Note::rest(Duration::Whole)];

        let xml = serialize_musicxml(&score).expect("serializes");
        assert!(xml.contains("<rest measure=\"yes\"/>"));
        assert!(xml.contains("<duration>1440</duration>"));
        let restored = crate::parse_musicxml(&xml).expect("reparses");
        let rest = &restored.parts[0].staves[0].measures[0].voices[0][0];
        assert!(rest.is_rest);
        assert!(matches!(rest.duration, Duration::Whole));
        assert_eq!(rest.dot_count, 0);
    }

    #[test]
    fn pitch_note_serialized_correctly() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::G, 4), Duration::Quarter)];
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<step>G</step>"));
        assert!(xml.contains("<octave>4</octave>"));
    }

    #[test]
    fn time_modification_tuplet_serializes_and_parses() {
        let mut score = Score::new("T", 120, 2, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Eighth);
        note.tuplet = Some(TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let xml = serialize_musicxml(&score).expect("MusicXML tuplet serialize");
        assert!(xml.contains("<actual-notes>3</actual-notes>"));
        let restored =
            super::super::parser::parse_musicxml(&xml).expect("serialized MusicXML tuplet parse");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].tuplet,
            score.parts[0].staves[0].measures[0].voices[0][0].tuplet
        );
    }

    fn score_with_note(
        articulations: Vec<acorde_core::Articulation>,
        arpeggiate: Option<bool>,
    ) -> Score {
        let mut score = Score::new("T", 120, 2, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Half);
        note.articulations = articulations;
        note.arpeggiate = arpeggiate;
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        score
    }

    #[test]
    fn mordent_serialized_in_ornaments() {
        use acorde_core::Articulation;
        let score = score_with_note(vec![Articulation::Mordent], None);
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<ornaments>"), "expected <ornaments> block");
        assert!(xml.contains("<mordent/>"), "expected <mordent/>");
    }

    #[test]
    fn turn_serialized_in_ornaments() {
        use acorde_core::Articulation;
        let score = score_with_note(vec![Articulation::Turn], None);
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<turn/>"));
    }

    #[test]
    fn tremolo_serialized_in_ornaments() {
        use acorde_core::Articulation;
        let score = score_with_note(vec![Articulation::Tremolo(3)], None);
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<tremolo>3</tremolo>"));
    }

    #[test]
    fn breath_mark_serialized() {
        use acorde_core::Articulation;
        let score = score_with_note(vec![Articulation::BreathMark], None);
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<breath-mark/>"));
    }

    #[test]
    fn caesura_serialized() {
        use acorde_core::Articulation;
        let score = score_with_note(vec![Articulation::Caesura], None);
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<caesura/>"));
    }

    #[test]
    fn arpeggiate_up_serialized() {
        let score = score_with_note(vec![], Some(true));
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<arpeggiate direction=\"up\"/>"));
    }

    #[test]
    fn arpeggiate_down_serialized() {
        let score = score_with_note(vec![], Some(false));
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<arpeggiate direction=\"down\"/>"));
    }

    #[test]
    fn glissando_cross_staff_roundtrip() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::with_microtone(Step::C, 4, 0, 50), Duration::Quarter);
        note.glissando_start = true;
        note.tab_position = Some(acorde_core::TabPosition { string: 2, fret: 3 });
        note.cross_staff = Some(acorde_core::CrossStaff {
            target_staff: 1,
            target_voice: Some(0),
        });
        score.parts[0]
            .staves
            .push(acorde_core::Staff::new(acorde_core::Clef::Bass));
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<staff>2</staff>"));
        assert!(xml.contains("<glissando number=\"1\" type=\"start\">"));
        let parsed = crate::musicxml::parser::parse_musicxml(&xml).unwrap();
        let parsed_note = &parsed.parts[0].staves[0].measures[0].voices[0][0];
        assert!(parsed_note.glissando_start);
        assert_eq!(
            parsed_note.cross_staff.as_ref().map(|c| c.target_staff),
            Some(1)
        );
        assert_eq!(parsed_note.pitches[0].microtone_cents, 50);
        assert_eq!(parsed_note.tab_position.as_ref().map(|p| p.fret), Some(3));
    }

    #[test]
    fn harp_pedals_serialize_and_reparse() {
        let mut score = Score::new("Harp", 120, 4, 4, 0, 1);
        let mut diagram = acorde_core::HarpPedalDiagram::default();
        diagram.positions[0] = acorde_core::HarpPedalPosition::Flat;
        diagram.positions[6] = acorde_core::HarpPedalPosition::Sharp;
        diagram.placement = Some("below".into());
        score.parts[0].staves[0].measures[0]
            .harp_pedal_diagrams
            .push(diagram);

        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<harp-pedals>"));
        assert!(xml.contains("<pedal-step>D</pedal-step><pedal-alter>-1</pedal-alter>"));
        let parsed = crate::musicxml::parser::parse_musicxml(&xml).unwrap();
        let diagram = &parsed.parts[0].staves[0].measures[0].harp_pedal_diagrams[0];
        assert_eq!(diagram.positions[0], acorde_core::HarpPedalPosition::Flat);
        assert_eq!(diagram.positions[6], acorde_core::HarpPedalPosition::Sharp);
    }
}
