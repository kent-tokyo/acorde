//! Bounded, renderer-independent spacing and system breaking in millimetres.

use serde::{Deserialize, Serialize};

/// A boundary following one measured item. Required and forbidden are hard constraints.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakConstraint {
    #[default]
    Allowed,
    Required,
    Forbidden,
}

/// Width of a measured object (a rhythmic gap or measure).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MeasuredSpacing {
    pub minimum_mm: f32,
    pub preferred_mm: f32,
}

/// Chosen contiguous input range and widths. `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredSystem {
    pub start: usize,
    pub end: usize,
    pub header_mm: f32,
    pub widths_mm: Vec<f32>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SpacingError {
    #[error("invalid measured width at index {index}")]
    InvalidMeasurement { index: usize },
    #[error("available width must be finite and positive")]
    InvalidAvailableWidth,
    #[error("{required_mm} mm of minimum spacing cannot fit in {available_mm} mm")]
    InsufficientWidth { required_mm: f32, available_mm: f32 },
    #[error("one header and break constraint are required for each measure")]
    MeasurementCount,
    #[error("measured breaking is limited to 100000 measures and 1..=128 measures per system")]
    Capacity,
    #[error("no legal system breaks fit the measured widths and hard constraints")]
    InfeasibleBreaks,
}

fn validate_widths(widths: &[MeasuredSpacing]) -> Result<(), SpacingError> {
    for (index, width) in widths.iter().enumerate() {
        if !width.minimum_mm.is_finite()
            || !width.preferred_mm.is_finite()
            || width.minimum_mm < 0.0
            || width.preferred_mm < width.minimum_mm
        {
            return Err(SpacingError::InvalidMeasurement { index });
        }
    }
    Ok(())
}

/// Fit measured items without ever compressing a minimum width.
///
/// Compression uses only the space between minimum and preferred widths. Expansion
/// is proportional to preferred widths (equal for all-zero inputs). The function
/// returns an error instead of silently scaling away a collision constraint.
pub fn allocate_measured_widths(
    widths: &[MeasuredSpacing],
    available_mm: f32,
) -> Result<Vec<f32>, SpacingError> {
    validate_widths(widths)?;
    if !available_mm.is_finite() || available_mm <= 0.0 {
        return Err(SpacingError::InvalidAvailableWidth);
    }
    if widths.is_empty() {
        return Ok(Vec::new());
    }
    let minimum: f64 = widths.iter().map(|width| f64::from(width.minimum_mm)).sum();
    let preferred: f64 = widths
        .iter()
        .map(|width| f64::from(width.preferred_mm))
        .sum();
    let available = f64::from(available_mm);
    if minimum > available {
        return Err(SpacingError::InsufficientWidth {
            required_mm: minimum as f32,
            available_mm,
        });
    }
    Ok(widths
        .iter()
        .map(|width| {
            let min = f64::from(width.minimum_mm);
            let ideal = f64::from(width.preferred_mm);
            let fitted = if preferred > available {
                min + (ideal - min) * (available - minimum) / (preferred - minimum)
            } else if preferred > 0.0 {
                ideal * available / preferred
            } else {
                available / widths.len() as f64
            };
            (fitted as f32).max(width.minimum_mm)
        })
        .collect())
}

/// Select system breaks with dynamic programming over measured widths.
///
/// `headers_mm[i]` is the header needed if a system starts at input measure `i`.
/// Widths exclude that header. Required breaks and forbidden boundaries (for example
/// a keep-together group) cannot be traded for a lower spacing cost. Work is bounded
/// by `measure_count * max_measures`. Ties deterministically favor the longer line.
/// The last line remains at preferred width when it is short.
pub fn break_measured_systems(
    widths: &[MeasuredSpacing],
    headers_mm: &[f32],
    constraints: &[BreakConstraint],
    first_width_mm: f32,
    width_mm: f32,
    max_measures: usize,
) -> Result<Vec<MeasuredSystem>, SpacingError> {
    break_measured_systems_with_columns(
        widths,
        headers_mm,
        constraints,
        first_width_mm,
        width_mm,
        max_measures,
        max_measures,
        &vec![1; widths.len()],
    )
}

/// Print's visual-column capacities keep a multi-rest indivisible, even when it
/// alone is larger than the requested line capacity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn break_measured_systems_with_columns(
    widths: &[MeasuredSpacing],
    headers_mm: &[f32],
    constraints: &[BreakConstraint],
    first_width_mm: f32,
    width_mm: f32,
    max_measures: usize,
    first_max: usize,
    columns: &[usize],
) -> Result<Vec<MeasuredSystem>, SpacingError> {
    validate_widths(widths)?;
    if headers_mm.len() != widths.len()
        || constraints.len() != widths.len()
        || columns.len() != widths.len()
        || columns.contains(&0)
    {
        return Err(SpacingError::MeasurementCount);
    }
    if !(1..=128).contains(&max_measures)
        || !(1..=128).contains(&first_max)
        || widths.len() > 100_000
    {
        return Err(SpacingError::Capacity);
    }
    if [first_width_mm, width_mm]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err(SpacingError::InvalidAvailableWidth);
    }
    for (index, header) in headers_mm.iter().enumerate() {
        if !header.is_finite() || *header < 0.0 {
            return Err(SpacingError::InvalidMeasurement { index });
        }
    }
    let count = widths.len();
    let mut costs = vec![f64::INFINITY; count + 1];
    let mut line_counts = vec![usize::MAX; count + 1];
    let mut next = vec![None; count];
    costs[count] = 0.0;
    line_counts[count] = 0;
    for start in (0..count).rev() {
        let total = if start == 0 { first_width_mm } else { width_mm };
        let available = f64::from(total) - f64::from(headers_mm[start]);
        if available <= 0.0 {
            continue;
        }
        let (mut minimum, mut preferred) = (0.0_f64, 0.0_f64);
        let capacity = if start == 0 { first_max } else { max_measures };
        let mut used_columns = 0_usize;
        for end in start..count.min(start.saturating_add(capacity)) {
            used_columns = used_columns.saturating_add(columns[end]);
            if end > start && used_columns > capacity {
                break;
            }
            minimum += f64::from(widths[end].minimum_mm);
            preferred += f64::from(widths[end].preferred_mm);
            if minimum > available {
                break;
            }
            if constraints[end] != BreakConstraint::Forbidden {
                let compression = if preferred > available {
                    (preferred - available) / (preferred - minimum).max(0.001)
                } else {
                    0.0
                };
                let expansion = if end + 1 == count {
                    0.0
                } else {
                    ((available - preferred) / available).max(0.0)
                };
                let cost =
                    1.0 + 4.0 * compression.powi(2) + 2.0 * expansion.powi(2) + costs[end + 1];
                let lines = line_counts[end + 1].saturating_add(1);
                // All feasible lines already preserve measured ink clearances.
                // Minimize the number of systems first, then their deformation;
                // an arbitrary stretch penalty must not create nearly empty pages.
                if cost.is_finite()
                    && (lines < line_counts[start]
                        || (lines == line_counts[start] && cost <= costs[start]))
                {
                    costs[start] = cost;
                    line_counts[start] = lines;
                    next[start] = Some(end + 1);
                }
            }
            if constraints[end] == BreakConstraint::Required {
                break;
            }
        }
    }
    let mut rows = Vec::new();
    let mut start = 0;
    while start < count {
        let end = next[start].ok_or(SpacingError::InfeasibleBreaks)?;
        let total = if start == 0 { first_width_mm } else { width_mm };
        let available = total - headers_mm[start];
        let preferred: f64 = widths[start..end]
            .iter()
            .map(|m| f64::from(m.preferred_mm))
            .sum();
        let target = if end == count && preferred > 0.0 {
            let rounded = preferred as f32;
            // Rounding a sum down by one ULP must not create an infeasible ragged
            // line when every item already has minimum == preferred.
            let upper = if f64::from(rounded) < preferred {
                f32::from_bits(rounded.to_bits().saturating_add(1))
            } else {
                rounded
            };
            available.min(upper)
        } else {
            available
        };
        rows.push(MeasuredSystem {
            start,
            end,
            header_mm: headers_mm[start],
            widths_mm: allocate_measured_widths(&widths[start..end], target)?,
        });
        start = end;
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_ragged_minimums_do_not_fail_from_rounding() {
        for count in 1..=128 {
            let widths = vec![
                MeasuredSpacing {
                    minimum_mm: 0.1,
                    preferred_mm: 0.1
                };
                count
            ];
            let rows = break_measured_systems(
                &widths,
                &vec![0.0; count],
                &vec![BreakConstraint::Allowed; count],
                100.0,
                100.0,
                128,
            )
            .unwrap();
            assert_eq!(rows.len(), 1);
            assert!(rows[0].widths_mm.iter().all(|&w| w >= 0.1));
        }
    }

    #[test]
    fn compresses_only_flexible_space_and_reports_infeasible_width() {
        let widths = [
            MeasuredSpacing {
                minimum_mm: 20.0,
                preferred_mm: 30.0,
            },
            MeasuredSpacing {
                minimum_mm: 10.0,
                preferred_mm: 30.0,
            },
        ];
        assert_eq!(
            allocate_measured_widths(&widths, 45.0).unwrap(),
            [25.0, 20.0]
        );
        assert_eq!(
            allocate_measured_widths(&widths, 30.0).unwrap(),
            [20.0, 10.0]
        );
        assert!(matches!(
            allocate_measured_widths(&widths, 29.0),
            Err(SpacingError::InsufficientWidth { .. })
        ));
    }

    #[test]
    fn hard_breaks_and_keep_together_survive_global_line_choice() {
        let widths = vec![
            MeasuredSpacing {
                minimum_mm: 20.0,
                preferred_mm: 30.0
            };
            5
        ];
        let rows = break_measured_systems(
            &widths,
            &[10.0; 5],
            &[
                BreakConstraint::Allowed,
                BreakConstraint::Required,
                BreakConstraint::Forbidden,
                BreakConstraint::Allowed,
                BreakConstraint::Allowed,
            ],
            80.0,
            100.0,
            8,
        )
        .unwrap();
        assert_eq!(
            rows.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>(),
            [(0, 2), (2, 5)]
        );
        for row in rows {
            assert!(row.widths_mm.iter().all(|width| *width >= 20.0));
        }
    }

    #[test]
    fn impossible_keep_together_is_an_error_not_a_scaled_line() {
        let widths = vec![
            MeasuredSpacing {
                minimum_mm: 40.0,
                preferred_mm: 50.0
            };
            3
        ];
        assert_eq!(
            break_measured_systems(
                &widths,
                &[10.0; 3],
                &[
                    BreakConstraint::Forbidden,
                    BreakConstraint::Forbidden,
                    BreakConstraint::Allowed,
                ],
                100.0,
                100.0,
                8
            ),
            Err(SpacingError::InfeasibleBreaks)
        );
    }

    #[test]
    fn rejects_invalid_input_and_honors_short_first_system() {
        let widths = vec![
            MeasuredSpacing {
                minimum_mm: 30.0,
                preferred_mm: 40.0
            };
            4
        ];
        let rows = break_measured_systems(
            &widths,
            &[10.0; 4],
            &[BreakConstraint::Allowed; 4],
            50.0,
            130.0,
            8,
        )
        .unwrap();
        assert_eq!(
            rows.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>(),
            [(0, 1), (1, 4)]
        );
        assert!(matches!(
            allocate_measured_widths(
                &[MeasuredSpacing {
                    minimum_mm: f32::NAN,
                    preferred_mm: 1.0
                }],
                10.0
            ),
            Err(SpacingError::InvalidMeasurement { .. })
        ));
        assert_eq!(
            break_measured_systems(&widths, &[], &[], 50.0, 100.0, 8),
            Err(SpacingError::MeasurementCount)
        );
        assert_eq!(
            break_measured_systems(
                &widths,
                &[10.0; 4],
                &[BreakConstraint::Allowed; 4],
                50.0,
                100.0,
                usize::MAX
            ),
            Err(SpacingError::Capacity)
        );
    }
}
