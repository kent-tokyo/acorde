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
    let measures = match score.parts.first()
        .and_then(|p| p.staves.first())
        .map(|s| &s.measures)
    {
        Some(m) => m,
        None => return vec![],
    };

    let n = measures.len();

    // Pre-scan: locate Fine, Segno, and Coda markers.
    let mut segno_idx: Option<usize> = None;
    let mut coda_idx:  Option<usize> = None;
    for (j, m) in measures.iter().enumerate() {
        match m.navigation.as_deref() {
            Some("Segno") => segno_idx = Some(j),
            Some("Coda")  => coda_idx  = Some(j),
            _ => {}
        }
    }

    let mut seq = Vec::with_capacity(n + 4);
    let mut i = 0usize;
    let mut repeat_start = 0usize;
    let mut volta_pass: u8 = 1;
    // Navigation-pass state (D.C./D.S. return).
    let mut in_nav_pass = false;
    let mut nav_fine    = false; // stop at Fine
    let mut nav_coda    = false; // jump to Coda at ToCoda

    while i < n {
        let m = &measures[i];

        // Volta / barline-repeat handling is suspended during a navigation pass.
        if !in_nav_pass {
            // Skip volta blocks that don't belong to the current pass.
            if let Some(volta) = &m.volta
                && volta.number != volta_pass
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
                Some("DaCapo")         => Some((0, false, false)),
                Some("DaCapoAlFine")   => Some((0, true,  false)),
                Some("DaCapoAlCoda")   => Some((0, false, true)),
                Some("DalSegno")       => segno_idx.map(|s| (s, false, false)),
                Some("DalSegnoAlFine") => segno_idx.map(|s| (s, true,  false)),
                Some("DalSegnoAlCoda") => segno_idx.map(|s| (s, false, true)),
                _ => None,
            };
            if let Some((target, fine, coda)) = jump {
                in_nav_pass = true;
                nav_fine    = fine;
                nav_coda    = coda;
                i = target;
                continue;
            }

            // Barline-repeat handling (only outside navigation pass).
            if matches!(m.barline_left, Barline::RepeatStart | Barline::RepeatBoth) {
                repeat_start = i;
            }

            match m.barline_right {
                Barline::RepeatEnd | Barline::RepeatBoth => {
                    if volta_pass == 1 {
                        volta_pass = 2;
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
    use crate::model::score::{Measure, Score, VoltaBracket};
    use crate::model::notation::Barline;

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
        m2.volta = Some(VoltaBracket { number: 1, kind: "begin_end".into() });
        m2.barline_right = Barline::RepeatEnd;
        let mut m3 = plain(4);
        m3.volta = Some(VoltaBracket { number: 2, kind: "begin_end".into() });

        let score = score_with_measures(vec![m0, m1, m2, m3]);
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 0, 1, 3]);
    }

    #[test]
    fn volta_second_pass_skips_ending_1() {
        let mut m0 = plain(1);
        m0.barline_left = Barline::RepeatStart;
        let m1 = plain(2);
        let mut m2 = plain(3);
        m2.volta = Some(VoltaBracket { number: 1, kind: "begin_end".into() });
        m2.barline_right = Barline::RepeatEnd;
        let mut m3 = plain(4);
        m3.volta = Some(VoltaBracket { number: 2, kind: "begin_end".into() });
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
        let score = score_with_measures(vec![
            plain(1), plain(2), with_nav(3, "DaCapo"),
        ]);
        // Pass: 0,1,2 → D.C. → in_nav_pass, jump to 0 → 0,1,2 → i=3 → done
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 0, 1, 2]);
    }

    #[test]
    fn da_capo_al_fine_stops_at_fine() {
        // [A Fine][B][C D.C.alFine] → A B C A (stop at Fine=0)
        let mut m0 = plain(1);
        m0.navigation = Some("Fine".into());
        let score = score_with_measures(vec![
            m0, plain(2), with_nav(3, "DaCapoAlFine"),
        ]);
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
            plain(1), m1, plain(3), m3, with_nav(5, "DalSegnoAlCoda"), m5, plain(7),
        ]);
        assert_eq!(measure_sequence(&score), vec![0, 1, 2, 3, 4, 1, 2, 3, 5, 6]);
    }
}
