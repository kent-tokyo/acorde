//! Typed-spanner endpoint maintenance shared by structural editing commands.

use super::{NotationSpanner, NotationSpannerKind, Note, NoteAddr, Score};

pub(super) fn note_at<'a>(score: &'a Score, address: &NoteAddr) -> Option<&'a Note> {
    score
        .parts
        .get(address.part)
        .and_then(|part| part.staves.get(address.staff))
        .and_then(|staff| staff.measures.get(address.measure))
        .and_then(|measure| measure.voices.get(address.voice))
        .and_then(|voice| voice.get(address.note))
}

fn note_at_mut<'a>(score: &'a mut Score, address: &NoteAddr) -> Option<&'a mut Note> {
    score
        .parts
        .get_mut(address.part)
        .and_then(|part| part.staves.get_mut(address.staff))
        .and_then(|staff| staff.measures.get_mut(address.measure))
        .and_then(|measure| measure.voices.get_mut(address.voice))
        .and_then(|voice| voice.get_mut(address.note))
}

pub(super) fn clear_legacy_spanner_endpoints(score: &mut Score, spanner: &NotationSpanner) {
    let clear_start = |note: &mut Note| match spanner.kind {
        NotationSpannerKind::Slur => note.slur_start = false,
        NotationSpannerKind::Glissando => note.glissando_start = false,
        NotationSpannerKind::TrillLine => note.trill_line_start = false,
        NotationSpannerKind::Pedal => note.pedal_start = false,
        NotationSpannerKind::Ottava => note.ottava_start = None,
    };
    let clear_end = |note: &mut Note| match spanner.kind {
        NotationSpannerKind::Slur => note.slur_end = false,
        NotationSpannerKind::Glissando => note.glissando_end = false,
        NotationSpannerKind::TrillLine => note.trill_line_end = false,
        NotationSpannerKind::Pedal => note.pedal_end = false,
        NotationSpannerKind::Ottava => note.ottava_end = false,
    };
    if let Some(note) = note_at_mut(score, &spanner.start) {
        clear_start(note);
    }
    if let Some(note) = note_at_mut(score, &spanner.end) {
        clear_end(note);
    }
}

/// Removes complete spans whose endpoints cannot be mapped to live notes.
pub(super) fn remap_spanners(
    score: &mut Score,
    mut remap: impl FnMut(&NoteAddr) -> Option<NoteAddr>,
) {
    let spanners = std::mem::take(&mut score.spanners);
    score.spanners = spanners
        .into_iter()
        .filter_map(|mut spanner| {
            spanner.start = remap(&spanner.start)?;
            spanner.end = remap(&spanner.end)?;
            (note_at(score, &spanner.start).is_some() && note_at(score, &spanner.end).is_some())
                .then_some(spanner)
        })
        .collect();
}

pub(super) fn prune_orphaned_spanners(score: &mut Score) {
    remap_spanners(score, |address| Some(address.clone()));
}

pub(super) fn capture_replaced_endpoint_ids(
    score: &Score,
    mut replaced: impl FnMut(&NoteAddr) -> bool,
) -> Vec<(NoteAddr, String)> {
    score
        .spanners
        .iter()
        .flat_map(|spanner| [&spanner.start, &spanner.end])
        .filter(|address| replaced(address))
        .filter_map(|address| {
            note_at(score, address).map(|note| (address.clone(), note.id.clone()))
        })
        .collect()
}

pub(super) fn remap_replaced_spanner_endpoints(
    score: &mut Score,
    endpoint_ids: &[(NoteAddr, String)],
) {
    let mapped: Vec<(NoteAddr, Option<NoteAddr>)> = endpoint_ids
        .iter()
        .map(|(address, note_id)| {
            let matches: Vec<usize> = score
                .parts
                .get(address.part)
                .and_then(|part| part.staves.get(address.staff))
                .and_then(|staff| staff.measures.get(address.measure))
                .and_then(|measure| measure.voices.get(address.voice))
                .map(|voice| {
                    voice
                        .iter()
                        .enumerate()
                        .filter_map(|(index, note)| (note.id == *note_id).then_some(index))
                        .collect()
                })
                .unwrap_or_default();
            let replacement = (matches.len() == 1).then(|| NoteAddr {
                note: matches[0],
                ..address.clone()
            });
            (address.clone(), replacement)
        })
        .collect();
    remap_spanners(score, |address| {
        mapped
            .iter()
            .find(|(old, _)| old == address)
            .map(|(_, replacement)| replacement.clone())
            .unwrap_or_else(|| Some(address.clone()))
    });
}
