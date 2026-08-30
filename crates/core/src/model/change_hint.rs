use serde::{Deserialize, Serialize};

/// Granularity of the change produced by a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeScope {
    /// Score-level change: metadata, global tempo, add/delete part, new score.
    Global,
    /// Only a specific part changed (by part index).
    Part(usize),
    /// One or more measures in a specific part+staff changed.
    Measures {
        part: usize,
        staff: usize,
        start: usize,
        end: usize,
    },
}

/// Lightweight hint returned by [`crate::ScoreEngine::apply`].
///
/// Consumers use this to decide which subsystems need updating (layout, audio)
/// without diffing the full score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeHint {
    pub scope: ChangeScope,
    /// True when the layout engine must be re-invoked (measure added/deleted,
    /// multi-rest count changed, clef or key/time sig change).
    pub layout_dirty: bool,
    /// True when playback events must be recomputed (pitch, duration, or tempo changed).
    pub playback_dirty: bool,
}

impl ChangeHint {
    /// Merge two hints, taking the broader scope and OR-ing dirty flags.
    pub fn merge(self, other: ChangeHint) -> ChangeHint {
        ChangeHint {
            scope: merge_scope(self.scope, other.scope),
            layout_dirty: self.layout_dirty || other.layout_dirty,
            playback_dirty: self.playback_dirty || other.playback_dirty,
        }
    }
}

fn merge_scope(a: ChangeScope, b: ChangeScope) -> ChangeScope {
    match (a, b) {
        (ChangeScope::Global, _) | (_, ChangeScope::Global) => ChangeScope::Global,
        (ChangeScope::Part(pa), ChangeScope::Part(pb)) if pa == pb => ChangeScope::Part(pa),
        (ChangeScope::Part(_), ChangeScope::Part(_)) => ChangeScope::Global,
        (
            ChangeScope::Measures {
                part: pa,
                staff: sa,
                start: s1,
                end: e1,
            },
            ChangeScope::Measures {
                part: pb,
                staff: sb,
                start: s2,
                end: e2,
            },
        ) if pa == pb && sa == sb => ChangeScope::Measures {
            part: pa,
            staff: sa,
            start: s1.min(s2),
            end: e1.max(e2),
        },
        _ => ChangeScope::Global,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hint(scope: ChangeScope, layout: bool, playback: bool) -> ChangeHint {
        ChangeHint {
            scope,
            layout_dirty: layout,
            playback_dirty: playback,
        }
    }

    #[test]
    fn merge_global_wins() {
        let a = hint(ChangeScope::Global, false, true);
        let b = hint(ChangeScope::Part(0), true, false);
        let m = a.merge(b);
        assert_eq!(m.scope, ChangeScope::Global);
        assert!(m.layout_dirty);
        assert!(m.playback_dirty);
    }

    #[test]
    fn merge_same_part_stays_part() {
        let a = hint(ChangeScope::Part(1), false, true);
        let b = hint(ChangeScope::Part(1), true, false);
        let m = a.merge(b);
        assert_eq!(m.scope, ChangeScope::Part(1));
    }

    #[test]
    fn merge_different_parts_becomes_global() {
        let a = hint(ChangeScope::Part(0), false, false);
        let b = hint(ChangeScope::Part(1), false, false);
        let m = a.merge(b);
        assert_eq!(m.scope, ChangeScope::Global);
    }

    #[test]
    fn merge_measures_same_staff_extends_range() {
        let a = hint(
            ChangeScope::Measures {
                part: 0,
                staff: 0,
                start: 2,
                end: 4,
            },
            false,
            true,
        );
        let b = hint(
            ChangeScope::Measures {
                part: 0,
                staff: 0,
                start: 1,
                end: 3,
            },
            false,
            true,
        );
        let m = a.merge(b);
        assert_eq!(
            m.scope,
            ChangeScope::Measures {
                part: 0,
                staff: 0,
                start: 1,
                end: 4
            }
        );
    }

    #[test]
    fn merge_measures_different_staff_becomes_global() {
        let a = hint(
            ChangeScope::Measures {
                part: 0,
                staff: 0,
                start: 0,
                end: 1,
            },
            false,
            true,
        );
        let b = hint(
            ChangeScope::Measures {
                part: 0,
                staff: 1,
                start: 0,
                end: 1,
            },
            false,
            true,
        );
        let m = a.merge(b);
        assert_eq!(m.scope, ChangeScope::Global);
    }
}
