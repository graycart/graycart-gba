//! Consent-gated GitHub issue filing (crash + Help → Report).
//!
//! File only on explicit Send/OK. Dismiss keeps any local dump and never uploads.

mod body;
mod consent_ui;
mod dialogs;
mod envelope;
mod github;
mod scrub;
mod token;

pub use consent_ui::{ConsentOutcome, install_panic_hook};
pub use dialogs::{ReportUi, collect_bug_defaults, show, write_fault_envelope};
pub use envelope::{crash_dir, load_pending};
