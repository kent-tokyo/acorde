//! Parsing for ABC score and staves voice-grouping directives.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node {
    Voice { id: String, floating: bool },
    Group { kind: GroupKind, members: Vec<Node> },
    Barline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GroupKind {
    Voices,
    Brace,
    Bracket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Directive {
    pub connect_by_default: bool,
    pub members: Vec<Node>,
}

/// Parse a complete directive. The caller validates voice ids against the parsed tune.
pub(super) fn parse(line: &str) -> Result<Option<Directive>, &'static str> {
    let line = line.trim();
    let (keyword, value) = match line.split_once(char::is_whitespace) {
        Some(pair) => pair,
        None => (line, ""),
    };
    let connect_by_default = match keyword {
        "%%score" => false,
        "%%staves" => true,
        _ => return Ok(None),
    };
    let value = value.split('%').next().unwrap_or_default();
    let mut parser = Parser {
        input: value,
        position: 0,
        depth: 0,
    };
    let members = parser.members(None)?;
    if members.is_empty() {
        return Err("empty voice grouping");
    }
    Ok(Some(Directive {
        connect_by_default,
        members,
    }))
}

struct Parser<'a> {
    input: &'a str,
    position: usize,
    depth: usize,
}

impl Parser<'_> {
    fn members(&mut self, closing: Option<char>) -> Result<Vec<Node>, &'static str> {
        if self.depth > 32 {
            return Err("voice grouping nesting exceeds the limit");
        }
        let mut result = Vec::new();
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
                continue;
            }
            if Some(ch) == closing {
                self.advance();
                if result.is_empty() || matches!(result.last(), Some(Node::Barline)) {
                    return Err("empty group or misplaced barline");
                }
                return Ok(result);
            }
            match ch {
                ')' | ']' | '}' => return Err("unmatched closing delimiter"),
                '(' | '[' | '{' => {
                    self.advance();
                    let (kind, end) = match ch {
                        '(' => (GroupKind::Voices, ')'),
                        '[' => (GroupKind::Bracket, ']'),
                        _ => (GroupKind::Brace, '}'),
                    };
                    self.depth += 1;
                    let members = self.members(Some(end))?;
                    self.depth -= 1;
                    if kind == GroupKind::Voices
                        && members.iter().any(|n| !matches!(n, Node::Voice { .. }))
                    {
                        return Err("voice group may contain only voice ids");
                    }
                    result.push(Node::Group { kind, members });
                }
                '|' => {
                    self.advance();
                    if result.is_empty() || matches!(result.last(), Some(Node::Barline)) {
                        return Err("misplaced barline");
                    }
                    result.push(Node::Barline);
                }
                _ => {
                    let floating = ch == '*';
                    if floating {
                        self.advance();
                    }
                    let start = self.position;
                    while self
                        .peek()
                        .is_some_and(|c| !c.is_whitespace() && !"()[]{}|*".contains(c))
                    {
                        self.advance();
                    }
                    if start == self.position {
                        return Err("missing voice id");
                    }
                    result.push(Node::Voice {
                        id: self.input[start..self.position].to_string(),
                        floating,
                    });
                }
            }
        }
        if closing.is_some() {
            return Err("unclosed group");
        }
        if matches!(result.last(), Some(Node::Barline)) {
            return Err("misplaced barline");
        }
        Ok(result)
    }

    fn peek(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }
    fn advance(&mut self) {
        if let Some(ch) = self.peek() {
            self.position += ch.len_utf8();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_groups_and_inverted_defaults() {
        let score = parse("%%score [{Vln1 | Vln2} | Vla | Vc]")
            .unwrap()
            .unwrap();
        assert!(!score.connect_by_default);
        assert!(matches!(
            &score.members[..],
            [Node::Group {
                kind: GroupKind::Bracket,
                ..
            }]
        ));
        let staves = parse("%%staves (S A) (T B)").unwrap().unwrap();
        assert!(staves.connect_by_default);
        assert_eq!(staves.members.len(), 2);
        assert_eq!(parse("% ordinary comment").unwrap(), None);
    }

    #[test]
    fn rejects_malformed_grouping() {
        for input in [
            "%%score",
            "%%score [S A",
            "%%score S |",
            "%%score (S | A)",
            "%%score S ** A",
            "%%score S ]",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct VoiceStaff {
    pub ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct GroupSpan {
    pub kind: GroupKind,
    pub first: usize,
    pub last: usize,
}

#[derive(Debug, Clone)]
pub(super) struct Grouping {
    pub staves: Vec<VoiceStaff>,
    pub groups: Vec<GroupSpan>,
    /// Each entry describes the boundary after the corresponding staff.
    pub connected: Vec<bool>,
    pub floating: Vec<String>,
}

impl Directive {
    pub(super) fn flatten(&self) -> Result<Grouping, &'static str> {
        let mut grouping = Grouping {
            staves: Vec::new(),
            groups: Vec::new(),
            connected: Vec::new(),
            floating: Vec::new(),
        };
        grouping.add_members(&self.members, self.connect_by_default)?;
        let mut seen = std::collections::BTreeSet::new();
        for id in grouping.staves.iter().flat_map(|staff| &staff.ids) {
            if !seen.insert(id) {
                return Err("duplicate voice id");
            }
        }
        if grouping.staves.len() > 32 {
            return Err("voice grouping exceeds the staff limit");
        }
        Ok(grouping)
    }
}

impl Grouping {
    fn add_members(&mut self, members: &[Node], default: bool) -> Result<(), &'static str> {
        let mut has_barline = false;
        let mut first = true;
        for member in members {
            if matches!(member, Node::Barline) {
                has_barline = true;
                continue;
            }
            if let Node::Voice { id, floating: true } = member {
                let previous = self
                    .staves
                    .last_mut()
                    .ok_or("floating voice has no preceding staff")?;
                if previous.ids.len() >= 4 {
                    return Err("more than four voices on a staff");
                }
                previous.ids.push(id.clone());
                self.floating.push(id.clone());
                continue;
            }
            if !first {
                self.connected.push(default ^ has_barline);
            }
            let begin = self.staves.len();
            match member {
                Node::Voice {
                    id,
                    floating: false,
                } => self.staves.push(VoiceStaff {
                    ids: vec![id.clone()],
                }),
                Node::Group {
                    kind: GroupKind::Voices,
                    members,
                } => {
                    let mut ids = Vec::new();
                    for voice in members {
                        if let Node::Voice { id, floating } = voice {
                            ids.push(id.clone());
                            if *floating {
                                self.floating.push(id.clone());
                            }
                        }
                    }
                    if ids.len() > 4 {
                        return Err("more than four voices on a staff");
                    }
                    self.staves.push(VoiceStaff { ids });
                }
                Node::Group { kind, members } => {
                    self.add_members(members, default)?;
                    let end = self.staves.len().checked_sub(1).ok_or("empty group")?;
                    self.groups.push(GroupSpan {
                        kind: *kind,
                        first: begin,
                        last: end,
                    });
                }
                Node::Barline | Node::Voice { floating: true, .. } => {
                    return Err("invalid voice grouping member");
                }
            }
            first = false;
            has_barline = false;
        }
        Ok(())
    }
}

#[cfg(test)]
mod flatten_tests {
    use super::*;

    #[test]
    fn flattens_nested_groups_and_boundaries() {
        let group = parse("%%score [{V1|V2} | V3 V4]")
            .unwrap()
            .unwrap()
            .flatten()
            .unwrap();
        assert_eq!(group.staves.len(), 4);
        assert_eq!(group.connected, [true, true, false]);
        assert!(
            group
                .groups
                .iter()
                .any(|g| g.kind == GroupKind::Brace && g.first == 0 && g.last == 1)
        );
        assert!(
            group
                .groups
                .iter()
                .any(|g| g.kind == GroupKind::Bracket && g.first == 0 && g.last == 3)
        );
    }

    #[test]
    fn rejects_duplicate_or_excessive_voices() {
        for text in ["%%score S S", "%%score (a b c d e)", "%%score *S A"] {
            assert!(parse(text).unwrap().unwrap().flatten().is_err(), "{text}");
        }
    }
}

/// Project the parsed voice parts onto the requested staves while keeping all note events.
/// Returns explanations for grouping details which cannot be represented by the score model.
pub(super) fn apply(
    score: &mut acorde_core::Score,
    voice_ids: &[String],
    grouping: &Grouping,
) -> Result<Vec<String>, &'static str> {
    use acorde_core::{PartGroup, PartGroupSymbol, StaffGroup};

    let mut indices = Vec::with_capacity(grouping.staves.len());
    for staff in &grouping.staves {
        let mut voices = Vec::with_capacity(staff.ids.len());
        for id in &staff.ids {
            voices.push(
                voice_ids
                    .iter()
                    .position(|known| known == id)
                    .ok_or("unknown voice id")?,
            );
        }
        indices.push(voices);
    }
    let mut brace_end = vec![None; grouping.staves.len()];
    for group in grouping
        .groups
        .iter()
        .filter(|g| g.kind == GroupKind::Brace)
    {
        if group.first == group.last {
            continue;
        }
        if (group.first..=group.last).any(|i| brace_end[i].is_some()) {
            return Err("overlapping brace groups are not supported");
        }
        for slot in &mut brace_end[group.first..=group.last] {
            *slot = Some(group.last);
        }
    }
    let old_parts = &score.parts;
    let mut parts = Vec::new();
    let mut addresses = Vec::with_capacity(indices.len());
    let mut warnings = Vec::new();
    let mut used = std::collections::BTreeSet::new();
    for (staff_index, voices) in indices.iter().enumerate() {
        let first_index = *voices.first().ok_or("empty voice group")?;
        let source = old_parts.get(first_index).ok_or("voice has no part")?;
        let mut staff = source.staves.first().ok_or("voice has no staff")?.clone();
        used.insert(first_index);
        for (voice_index, &part_index) in voices.iter().enumerate().skip(1) {
            let other = old_parts
                .get(part_index)
                .and_then(|part| part.staves.first())
                .ok_or("voice has no staff")?;
            if staff.measures.len() != other.measures.len() {
                return Err("voices sharing a staff have different measure counts");
            }
            if staff.clef != other.clef || staff.transpose_semitones != other.transpose_semitones {
                warnings.push(format!(
                    "voice {} has staff properties that differ from the first voice",
                    grouping.staves[staff_index].ids[voice_index]
                ));
            }
            for (destination, source) in staff.measures.iter_mut().zip(&other.measures) {
                // Measure-level fields cannot be attached to an individual voice. Keep the
                // original parts intact if merging would silently discard different metadata.
                let mut destination_metadata = destination.clone();
                let mut source_metadata = source.clone();
                for voice in &mut destination_metadata.voices {
                    voice.clear();
                }
                for voice in &mut source_metadata.voices {
                    voice.clear();
                }
                if serde_json::to_value(&destination_metadata).ok()
                    != serde_json::to_value(&source_metadata).ok()
                {
                    return Err("voices sharing a staff have different measure properties");
                }
                destination.voices[voice_index] = source.voices[0].clone();
            }
            used.insert(part_index);
        }
        let continuation = staff_index > 0
            && brace_end[staff_index].is_some()
            && brace_end[staff_index - 1] == brace_end[staff_index];
        if continuation {
            let part: &mut acorde_core::Part = parts.last_mut().ok_or("missing braced part")?;
            let index = part.staves.len();
            part.staves.push(staff);
            addresses.push((parts.len() - 1, index));
        } else {
            let mut part = source.clone();
            part.staves = vec![staff];
            part.staff_groups.clear();
            addresses.push((parts.len(), 0));
            parts.push(part);
        }
    }
    for group in &grouping.groups {
        let (first_part, first_staff) = addresses[group.first];
        let (last_part, last_staff) = addresses[group.last];
        let symbol = match group.kind {
            GroupKind::Brace => PartGroupSymbol::Brace,
            GroupKind::Bracket => PartGroupSymbol::Bracket,
            GroupKind::Voices => continue,
        };
        if first_part == last_part {
            parts[first_part].staff_groups.push(StaffGroup {
                first_staff,
                last_staff,
                symbol,
                barlines_connect: grouping.connected[group.first..group.last]
                    .iter()
                    .all(|&value| value),
            });
        } else if first_staff == 0 && last_staff + 1 == parts[last_part].staves.len() {
            // Added below after replacing score.parts.
        } else {
            warnings.push(format!(
                "group at staves {}–{} crosses part boundaries",
                group.first + 1,
                group.last + 1
            ));
        }
    }
    let mut part_groups = Vec::new();
    for group in &grouping.groups {
        if group.kind != GroupKind::Bracket {
            continue;
        }
        let (first_part, first_staff) = addresses[group.first];
        let (last_part, last_staff) = addresses[group.last];
        if first_part != last_part
            && first_staff == 0
            && last_staff + 1 == parts[last_part].staves.len()
        {
            part_groups.push(PartGroup {
                first_part,
                last_part,
                symbol: PartGroupSymbol::Bracket,
                barlines_connect: grouping.connected[group.first..group.last]
                    .iter()
                    .all(|&value| value),
            });
        }
    }
    for (index, connected) in grouping.connected.iter().copied().enumerate() {
        if !connected {
            continue;
        }
        let covered = grouping.groups.iter().any(|group| {
            group.kind != GroupKind::Voices
                && group.first <= index
                && index < group.last
                && grouping.connected[group.first..group.last]
                    .iter()
                    .all(|&value| value)
                && {
                    let (first_part, first_staff) = addresses[group.first];
                    let (last_part, last_staff) = addresses[group.last];
                    first_part == last_part
                        || (first_staff == 0
                            && last_staff + 1 == parts[last_part].staves.len()
                            && group.kind == GroupKind::Bracket)
                }
        });
        if covered {
            continue;
        }
        let (left_part, left_staff) = addresses[index];
        let (right_part, right_staff) = addresses[index + 1];
        if left_part == right_part {
            parts[left_part].staff_groups.push(StaffGroup {
                first_staff: left_staff,
                last_staff: right_staff,
                symbol: PartGroupSymbol::None,
                barlines_connect: true,
            });
        } else if parts[left_part].staves.len() == 1 && parts[right_part].staves.len() == 1 {
            part_groups.push(PartGroup {
                first_part: left_part,
                last_part: right_part,
                symbol: PartGroupSymbol::None,
                barlines_connect: true,
            });
        } else {
            warnings.push(format!("barline between staves {} and {} cannot be represented without joining other staves", index + 1, index + 2));
        }
    }
    for (index, source) in old_parts.iter().enumerate() {
        if !used.contains(&index) {
            let id = voice_ids.get(index).map_or("(unnamed)", String::as_str);
            warnings.push(format!("voice {id} is omitted from the printed score"));
            let mut hidden = source.clone();
            for staff in &mut hidden.staves {
                staff.presentation.visible = false;
            }
            parts.push(hidden);
        }
    }
    for id in &grouping.floating {
        warnings.push(format!(
            "floating voice {id} is placed on the preceding staff"
        ));
    }
    score.parts = parts;
    score.part_groups = part_groups;
    Ok(warnings)
}
