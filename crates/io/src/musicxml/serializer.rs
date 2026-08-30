use crate::Error;
use acorde_core::{
    Articulation, Barline, GuitarTechnique, HairpinKind, Note, NoteHead, PartGroup,
    PartGroupSymbol, Score,
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

    for part in &score.parts {
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

            // Attributes
            if i == 0 {
                xml.push_str("      <attributes>\n");
                xml.push_str(&format!("        <divisions>{}</divisions>\n", DIVISIONS));
                let key = measure
                    .key_sig
                    .as_ref()
                    .unwrap_or(&score.settings.key_signature);
                xml.push_str("        <key>\n");
                xml.push_str(&format!("          <fifths>{}</fifths>\n", key.fifths));
                xml.push_str(&format!("          <mode>{}</mode>\n", key.mode));
                xml.push_str("        </key>\n");
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
                if staff.transpose_semitones != 0 {
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
                xml.push_str("      <direction placement=\"above\">\n");
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
                xml.push_str("      <direction placement=\"above\">\n");
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!("          <words>{}</words>\n", escape_xml(text)));
                xml.push_str("        </direction-type>\n");
                xml.push_str("      </direction>\n");
            }

            // Expression text (dolce, espressivo, etc. — no <sound> element)
            if let Some(text) = &measure.expression_text {
                xml.push_str("      <direction placement=\"above\">\n");
                xml.push_str("        <direction-type>\n");
                xml.push_str(&format!("          <words>{}</words>\n", escape_xml(text)));
                xml.push_str("        </direction-type>\n");
                xml.push_str("      </direction>\n");
            }

            // Notes (voice 0)
            for note in &measure.voices[0] {
                serialize_note(&mut xml, note);
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

fn serialize_note(xml: &mut String, note: &Note) {
    // Pedal start
    if note.pedal_start {
        xml.push_str("      <direction placement=\"below\">\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str("          <pedal type=\"start\" line=\"no\"/>\n");
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }

    // Ottava start
    if let Some(ok) = &note.ottava_start {
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
        xml.push_str("      <harmony>\n");
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

    let dur_ticks = note.duration.to_ticks(note.dot_count);

    if note.is_rest {
        xml.push_str("      <note>\n");
        xml.push_str("        <rest/>\n");
        xml.push_str(&format!("        <duration>{}</duration>\n", dur_ticks));
        xml.push_str("        <voice>1</voice>\n");
        xml.push_str(&format!(
            "        <type>{}</type>\n",
            note.duration.to_musicxml_type()
        ));
        for _ in 0..note.dot_count {
            xml.push_str("        <dot/>\n");
        }
        xml.push_str("      </note>\n");
    } else if let Some(pitch) = note.pitches.first() {
        xml.push_str("      <note>\n");
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
        xml.push_str("        <pitch>\n");
        xml.push_str(&format!(
            "          <step>{}</step>\n",
            pitch.step.to_char()
        ));
        if pitch.alter != 0 {
            xml.push_str(&format!("          <alter>{}</alter>\n", pitch.alter));
        }
        xml.push_str(&format!("          <octave>{}</octave>\n", pitch.octave));
        xml.push_str("        </pitch>\n");
        if !note.is_grace {
            xml.push_str(&format!("        <duration>{}</duration>\n", dur_ticks));
        }
        if note.tie_end {
            xml.push_str("        <tie type=\"stop\"/>\n");
        }
        if note.tie_start {
            xml.push_str("        <tie type=\"start\"/>\n");
        }
        xml.push_str("        <voice>1</voice>\n");
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
        serialize_notations(xml, note);
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
        for extra in note.pitches.iter().skip(1) {
            xml.push_str("      <note>\n");
            xml.push_str("        <chord/>\n");
            xml.push_str("        <pitch>\n");
            xml.push_str(&format!(
                "          <step>{}</step>\n",
                extra.step.to_char()
            ));
            if extra.alter != 0 {
                xml.push_str(&format!("          <alter>{}</alter>\n", extra.alter));
            }
            xml.push_str(&format!("          <octave>{}</octave>\n", extra.octave));
            xml.push_str("        </pitch>\n");
            xml.push_str(&format!("        <duration>{}</duration>\n", dur_ticks));
            xml.push_str("        <voice>1</voice>\n");
            xml.push_str(&format!(
                "        <type>{}</type>\n",
                note.duration.to_musicxml_type()
            ));
            xml.push_str("      </note>\n");
        }
    }

    // Pedal stop
    if note.pedal_end {
        xml.push_str("      <direction placement=\"below\">\n");
        xml.push_str("        <direction-type>\n");
        xml.push_str("          <pedal type=\"stop\" line=\"no\"/>\n");
        xml.push_str("        </direction-type>\n");
        xml.push_str("      </direction>\n");
    }

    // Ottava stop
    if note.ottava_end {
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

fn serialize_notations(xml: &mut String, note: &Note) {
    let has_tie = note.tie_start || note.tie_end;
    let has_slur = note.slur_start || note.slur_end;
    let has_trill_line = note.trill_line_start || note.trill_line_end;
    let has_artic = !note.articulations.is_empty();
    let has_arp = note.arpeggiate.is_some();
    let has_technical = note.fingering.is_some()
        || note.string_number.is_some()
        || note.technique_text.is_some()
        || note.guitar_technique.is_some();
    if !has_tie && !has_slur && !has_trill_line && !has_artic && !has_arp && !has_technical {
        return;
    }

    xml.push_str("        <notations>\n");
    if note.tie_end {
        xml.push_str("          <tied type=\"stop\"/>\n");
    }
    if note.tie_start {
        xml.push_str("          <tied type=\"start\"/>\n");
    }
    if note.slur_end {
        xml.push_str("          <slur number=\"1\" type=\"stop\"/>\n");
    }
    if note.slur_start {
        xml.push_str("          <slur number=\"1\" type=\"start\"/>\n");
    }
    if note.trill_line_end {
        xml.push_str("          <wavy-line number=\"1\" type=\"stop\"/>\n");
    }
    if note.trill_line_start {
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
        if let Some(f) = note.fingering {
            xml.push_str(&format!("            <fingering>{}</fingering>\n", f));
        }
        if let Some(s) = note.string_number {
            xml.push_str(&format!("            <string>{}</string>\n", s));
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
                    xml.push_str("            <bend><bend-alter>0</bend-alter></bend>\n")
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
    use acorde_core::{Duration, Note, Pitch, Score, Step};

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
    fn pitch_note_serialized_correctly() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::G, 4), Duration::Quarter)];
        let xml = serialize_musicxml(&score).unwrap();
        assert!(xml.contains("<step>G</step>"));
        assert!(xml.contains("<octave>4</octave>"));
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
}
