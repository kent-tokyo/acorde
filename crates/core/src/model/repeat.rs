use super::notation::Barline;
use super::score::Score;

/// Returns the ordered list of physical measure indices to play back,
/// expanding repeat sections, volta brackets, and navigation marks.
///
/// Handles:
/// - `RepeatStart` / `RepeatEnd` / `RepeatBoth` barlines
/// - First/second-ending volta brackets
/// - `DaCapo`, `DaCapoAlFine`, `DaCapoAlCoda`
/// - `DalSegno`, `DalSegnoAlFine`, `DalSegnoAlCoda`
/// - `Fine`, `ToCoda`, `Coda` markers
///
/// Limitations: single-level repeats; during a D.C./D.S. return pass, all
/// barline repeats and voltas are ignored (safe linear fallback).
pub fn measure_sequence(score: &Score) -> Vec<usize> {
    let measures = match score
        .parts
        .first()
        .and_then(|p| p.staves.first())
        .map(|s| &s.measures)
    {
        Some(m) => m,
        None => return vec![],
    };

    let n = measures.len();

    // Pre-scan: locate Fine, Segno, and Coda markers.
    let mut segno_idx: Option<usize> = None;
    let mut coda_idx: Option<usize> = None;
    for (j, m) in measures.iter().enumerate() {
        match m.navigation.as_deref() {
            Some("Segno") => segno_idx = Some(j),
            Some("Coda") => coda_idx = Some(j),
            _ => {}
        }
    }

    let mut seq = Vec::with_capacity(n + 4);
    // Each repeat end and each D.C./D.S. mark jumps back once. Without this, marks that do not
    // pair up (a stray first-ending end with its own repeat, a coda before its D.C.) sent
    // playback round the same bars forever, growing the sequence until allocation failed.
    let mut repeat_jumps = vec![0u8; n];
    let mut jump_taken = vec![false; n];
    // Backstop for any other unforeseen cycle: no real score plays a bar this many times.
    let limit = n.saturating_mul(16).max(64);
    let mut i = 0usize;
    let mut repeat_start = 0usize;
    let mut volta_pass: u8 = 1;
    // Navigation-pass state (D.C./D.S. return).
    let mut in_nav_pass = false;
    let mut nav_fine = false; // stop at Fine
    let mut nav_coda = false; // jump to Coda at ToCoda

    while i < n && seq.len() < limit {
        let m = &measures[i];

        // Volta / barline-repeat handling is suspended during a navigation pass.
        if !in_nav_pass {
            // Skip volta blocks that don't belong to the current pass.
            if let Some(volta) = &m.volta
                && !volta.plays_on(volta_pass)
                && (volta.kind == "begin" || volta.kind == "begin_end")
            {
                if volta.kind == "begin_end" {
                    i += 1;
                    continue;
                } else {
                    i += 1;
                    while i < n {
                        if let Some(v) = &measures[i].volta
                            && v.kind == "end"
                        {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
            }
        }

        seq.push(i);

        // Navigation-pass checks (Fine / ToCoda).
        if in_nav_pass {
            if nav_fine && matches!(m.navigation.as_deref(), Some("Fine")) {
                break;
            }
            if nav_coda
                && matches!(m.navigation.as_deref(), Some("ToCoda"))
                && let Some(ci) = coda_idx
            {
                i = ci;
                in_nav_pass = false;
                continue;
            }
        } else {
            // Check for D.C./D.S. marks.
            let jump: Option<(usize, bool, bool)> = match m.navigation.as_deref() {
                Some("DaCapo") => Some((0, false, false)),
                Some("DaCapoAlFine") => Some((0, true, false)),
                Some("DaCapoAlCoda") => Some((0, false, true)),
                Some("DalSegno") => segno_idx.map(|s| (s, false, false)),
                Some("DalSegnoAlFine") => segno_idx.map(|s| (s, true, false)),
                Some("DalSegnoAlCoda") => segno_idx.map(|s| (s, false, true)),
                _ => None,
            };
            if let Some((target, fine, coda)) = jump
                && !std::mem::replace(&mut jump_taken[i], true)
            {
                in_nav_pass = true;
                nav_fine = fine;
                nav_coda = coda;
                i = target;
                continue;
            }

            // Barline-repeat handling (only outside navigation pass).
            if matches!(m.barline_left, Barline::RepeatStart | Barline::RepeatBoth) {
                repeat_start = i;
            }

            match m.barline_right {
                Barline::RepeatEnd | Barline::RepeatBoth => {
                    // Inside an ending, repeat until the last ending's pass is reached: a
                    // `1, 2.` ending before a `3.` one plays the section three times.
                    let last_pass = if m.volta.is_some() {
                        let mut last = volta_pass;
                        let mut j = repeat_start;
                        while j < n && (j <= i || measures[j].volta.is_some()) {
                            if let Some(volta) = &measures[j].volta {
                                last = last.max(volta.passes().into_iter().max().unwrap_or(1));
                            }
                            j += 1;
                        }
                        last
                    } else {
                        2
                    };
                    // Each repeat end jumps back at most once per extra pass, so marks that do
                    // not pair up cannot loop.
                    let again = if m.volta.is_some() {
                        volta_pass < last_pass
                            && usize::from(repeat_jumps[i]) + 1 < usize::from(last_pass)
                    } else {
                        repeat_jumps[i] == 0
                    };
                    if again {
                        repeat_jumps[i] = repeat_jumps[i].saturating_add(1);
                        volta_pass = volta_pass.saturating_add(1).min(last_pass);
                        i = repeat_start;
                    } else {
                        volta_pass = 1;
                        if matches!(m.barline_right, Barline::RepeatBoth) {
                            repeat_start = i + 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                _ => {}
            }
        }

        i += 1;
    }

    seq
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::notation::Barline;
    use crate::model::score::{Measure, Score, VoltaBracket};

    fn score_with_measures(measures: Vec<Measure>) -> Score {
        let mut score = Score::new("T", 120, 4, 4, 0, 0);
        score.parts[0].staves[0].measures = measures;
        score
    }

    fn plain(n: u32) -> Measure {
        let mut m = Measure::empty(4, 4);
        m.number = n;
        m
    }

    fn with_nav(n: u32, nav: &str) -> Measure {
        let mut m = plain(n);
        m.navigation = Some(nav.to_string());
        m
    }

    #[test]
    fn an_ending_for_several_passes_repeats_the_section_that_many_times() {
        // |: A | 1, 2. B :| 3. C |
        let mut a = plain(1);
        a.barline_left = Barline::RepeatStart;
        let mut b = plain(2);
        b.barline_right = Barline::RepeatEnd;
        b.volta = Some(VoltaBracket {
            number: 1,
            kind: "begin_end".into(),
            numbers: vec![1, 2],
        });
        let mut c = plain(3);
        c.volta = Some(VoltaBracket {
            number: 3,
            kind: "begin_end".into(),
            numbers: Vec::new(),
        });
        let score = score_with_measures(vec![a, b, c]);
        assert_eq!(measure_sequence(&score), vec![0, 1, 0, 1, 0, 2]);
        assert_eq!(VoltaBracket::parse_passes("1, 2"), vec![1, 2]);
        assert_eq!(VoltaBracket::parse_passes("1-3"), vec![1, 2, 3]);
        let three = VoltaBracket {
            number: 1,
            kind: "begin".into(),
            numbers: vec![1, 2, 3],
        };
        assert_eq!(three.label(), "1-3");
    }

    #[test]
    fn unpaired_repeat_marks_play_each_repeat_once_instead_of_looping() {
        // A first ending that only ends (its start is missing) with a repeat, a second ending,
        // then a later repeat end with another second ending: the two repeat ends used to reset
        // each other's pass and loop forever.
        let volta = |number, kind: &str| {
            Some(VoltaBracket {
                number,
                kind: kind.into(),
                numbers: Vec::new(),
            })
        };
        let mut m1 = plain(2);
        m1.barline_right = Barline::RepeatEnd;
        m1.volta = volta(1, "end");
        let mut m2 = plain(3);
        m2.volta = volta(2, "begin_end");
        let mut m3 = plain(4);
        m3.barline_right = Barline::RepeatEnd;
        m3.volta = volta(2, "end");
        let mut m4 = plain(5);
        m4.volta = volta(2, "begin_end");
        let score = score_with_measures(vec![plain(1), m1, m2, m3, m4]);
        let sequence = measure_sequence(&score);
        assert!(sequence.len() < 20, "{sequence:?}");
        assert_eq!(sequence.iter().filter(|&&bar| bar == 3).count(), 2);
    }

    #[test]
    fn a_coda_before_its_da_capo_does_not_loop() {
        let score = score_with_measures(vec![
            with_nav(1, "Coda"),
            with_nav(2, "ToCoda"),
            with_nav(3, "DaCapoAlCoda"),
        ]);
        let sequence = measure_sequence(&score);
        assert!(sequence.len() < 20, "{sequence:?}");
    }

    #[test]
    fn no_repeat_is_linear() {
        let score = score_with_measures(vec![plain(1), plain(2), plain(3), plain(4)]);
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 3]);
    }

    #[test]
    fn simple_repeat_doubles_section() {
        let mut m0 = plain(1);
        m0.barline_left = Barline::RepeatStart;
        let m1 = plain(2);
        let mut m2 = plain(3);
        m2.barline_right = Barline::RepeatEnd;
        let m3 = plain(4);

        let score = score_with_measures(vec![m0, m1, m2, m3]);
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 0, 1, 2, 3]);
    }

    #[test]
    fn volta_first_pass_plays_ending_1() {
        let mut m0 = plain(1);
        m0.barline_left = Barline::RepeatStart;
        let m1 = plain(2);
        let mut m2 = plain(3);
        m2.volta = Some(VoltaBracket {
            number: 1,
            kind: "begin_end".into(),
            numbers: Vec::new(),
        });
        m2.barline_right = Barline::RepeatEnd;
        let mut m3 = plain(4);
        m3.volta = Some(VoltaBracket {
            number: 2,
            kind: "begin_end".into(),
            numbers: Vec::new(),
        });

        let score = score_with_measures(vec![m0, m1, m2, m3]);
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 0, 1, 3]);
    }

    #[test]
    fn volta_second_pass_skips_ending_1() {
        let mut m0 = plain(1);
        m0.barline_left = Barline::RepeatStart;
        let m1 = plain(2);
        let mut m2 = plain(3);
        m2.volta = Some(VoltaBracket {
            number: 1,
            kind: "begin_end".into(),
            numbers: Vec::new(),
        });
        m2.barline_right = Barline::RepeatEnd;
        let mut m3 = plain(4);
        m3.volta = Some(VoltaBracket {
            number: 2,
            kind: "begin_end".into(),
            numbers: Vec::new(),
        });
        let m4 = plain(5);

        let score = score_with_measures(vec![m0, m1, m2, m3, m4]);
        let seq = measure_sequence(&score);
        assert_eq!(seq, vec![0, 1, 2, 0, 1, 3, 4]);
        let second_pass_third = seq[5];
        assert_eq!(second_pass_third, 3);
    }

    // ── Navigation marks ─────────────────────────────────────────────────────

    #[test]
    fn da_capo_jumps_to_start() {
        // [A][B][C D.C.] → A B C A B C ...
        // The second D.C. encounter is ignored (in_nav_pass=true) so we play A B C straight.
        let score = score_with_measures(vec![plain(1), plain(2), with_nav(3, "DaCapo")]);
        // Pass: 0,1,2 → D.C. → in_nav_pass, jump to 0 → 0,1,2 → i=3 → done
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 0, 1, 2]);
    }

    #[test]
    fn da_capo_al_fine_stops_at_fine() {
        // [A Fine][B][C D.C.alFine] → A B C A (stop at Fine=0)
        let mut m0 = plain(1);
        m0.navigation = Some("Fine".into());
        let score = score_with_measures(vec![m0, plain(2), with_nav(3, "DaCapoAlFine")]);
        // Pass: 0,1,2 → D.C.alFine → jump to 0 → push 0 → Fine found → break
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 0]);
    }

    #[test]
    fn dal_segno_al_coda_jumps_to_coda() {
        // [A][B Segno][C][D ToCoda][E D.S.alCoda][F Coda][G]
        // Pass: 0,1,2,3,4 → D.S.alCoda → jump to segno(1) → in_nav_pass
        // → push 1,2,3 → ToCoda at 3 → jump to coda(5) → push 5,6
        let mut m1 = plain(2);
        m1.navigation = Some("Segno".into());
        let mut m3 = plain(4);
        m3.navigation = Some("ToCoda".into());
        let mut m5 = plain(6);
        m5.navigation = Some("Coda".into());
        let score = score_with_measures(vec![
            plain(1),
            m1,
            plain(3),
            m3,
            with_nav(5, "DalSegnoAlCoda"),
            m5,
            plain(7),
        ]);
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 3, 4, 1, 2, 3, 5, 6]);
    }
}
