use acorde_core::Score;
use serde::{Deserialize, Serialize};

/// Version of the serialized import/export report contract.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

fn default_report_schema_version() -> u32 {
    REPORT_SCHEMA_VERSION
}

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

    pub fn warning(code: impl Into<String>, loss_reason: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Warning,
            source_location: None,
            preserved_value: None,
            loss_reason: Some(loss_reason.into()),
        }
    }

    pub fn is_loss(&self) -> bool {
        self.loss_reason.is_some()
    }
}

/// Result of importing a notation document, including conversion diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    /// Version of the serialized report shape.
    #[serde(default = "default_report_schema_version")]
    pub schema_version: u32,
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
            schema_version: REPORT_SCHEMA_VERSION,
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

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .count()
    }

    pub fn loss_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_loss())
            .count()
    }
}

/// Result of exporting a notation document, including conversion diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportReport<T> {
    /// Version of the serialized report shape.
    #[serde(default = "default_report_schema_version")]
    pub schema_version: u32,
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
            schema_version: REPORT_SCHEMA_VERSION,
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

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .count()
    }

    pub fn loss_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_loss())
            .count()
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
        assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
        assert_eq!(report.format, "unknown");
        assert!(report.diagnostics.is_empty());
        let output = ExportReport::new("output");
        assert_eq!(output.schema_version, REPORT_SCHEMA_VERSION);
        assert_eq!(output.format, "unknown");
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn legacy_export_report_defaults_to_current_schema() {
        let restored: ExportReport<String> =
            serde_json::from_str(r#"{"format":"musicxml","output":"output","diagnostics":[]}"#)
                .expect("legacy report parses");
        assert_eq!(restored.schema_version, REPORT_SCHEMA_VERSION);
    }

    #[test]
    fn report_counts_classify_diagnostics() {
        let mut report = ImportReport::new(Score::default());
        report.diagnostics.push(Diagnostic::info("kept", "value"));
        report
            .diagnostics
            .push(Diagnostic::warning("lost", "not represented"));
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.loss_count(), 1);

        let mut export = ExportReport::new("output");
        export
            .diagnostics
            .push(Diagnostic::warning("lost", "not represented"));
        assert_eq!(export.warning_count(), 1);
        assert_eq!(export.loss_count(), 1);
    }
}
