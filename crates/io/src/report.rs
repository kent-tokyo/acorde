use acorde_core::Score;
use serde::{Deserialize, Serialize};

/// Severity of an interchange diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// A source-grounded note about an import or export decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserved_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_reason: Option<String>,
}

impl Diagnostic {
    pub fn info(code: impl Into<String>, preserved_value: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Info,
            source_location: None,
            preserved_value: Some(preserved_value.into()),
            loss_reason: None,
        }
    }
}

/// Result of importing a notation document, including conversion diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    /// Stable lowercase format identifier for the source document.
    #[serde(default)]
    pub format: String,
    pub score: Score,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl ImportReport {
    pub fn new(score: Score) -> Self {
        Self {
            format: "unknown".to_string(),
            score,
            diagnostics: Vec::new(),
        }
    }

    pub fn for_format(score: Score, format: impl Into<String>) -> Self {
        Self {
            format: format.into(),
            ..Self::new(score)
        }
    }
}

/// Result of exporting a notation document, including conversion diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportReport<T> {
    /// Stable lowercase format identifier for the output document.
    #[serde(default)]
    pub format: String,
    pub output: T,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> ExportReport<T> {
    pub fn new(output: T) -> Self {
        Self {
            format: "unknown".to_string(),
            output,
            diagnostics: Vec::new(),
        }
    }

    pub fn for_format(output: T, format: impl Into<String>) -> Self {
        Self {
            format: format.into(),
            ..Self::new(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_round_trips_with_optional_fields() {
        let mut diagnostic = Diagnostic::info("musicxml.title", "A score");
        diagnostic.source_location = Some("/score-work/title".to_string());
        let json = serde_json::to_string(&diagnostic).expect("diagnostic serializes");
        let restored: Diagnostic = serde_json::from_str(&json).expect("diagnostic parses");
        assert_eq!(restored, diagnostic);
    }

    #[test]
    fn reports_default_to_no_loss() {
        let report = ImportReport::new(Score::default());
        assert_eq!(report.format, "unknown");
        assert!(report.diagnostics.is_empty());
        let output = ExportReport::new("output");
        assert_eq!(output.format, "unknown");
        assert!(output.diagnostics.is_empty());
    }
}
