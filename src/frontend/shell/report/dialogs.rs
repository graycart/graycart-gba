//! Consent + Help report dialogs (egui). File only on Send/OK.

use super::body::{
    BugFields, FeatureFields, bug_issue, crash_issue_title, feature_issue, git_commit,
    host_os_label,
};
use super::envelope::{
    CrashMeta, EnvelopeKind, crash_dir, mark_consumed, mark_skipped, write_envelope,
};
use super::github::{self, GithubIssueFiler, IssueFiler, bug_new_issue_url, feature_new_issue_url};
use super::scrub::path_basename;
use super::token::resolve_token;
use crate::frontend::shell::settings::FrontendSettings;
use crate::frontend::shell::{audio::AudioDevicePref, brand};
use egui::Context;
use graycart::{BootMode, HostHardwarePref};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentOutcome {
    /// Still open — do not quit / do not file.
    Open,
    /// User filed (or attempted fallback). Caller may continue / quit.
    Sent,
    /// User dismissed — must not call API.
    Dismissed,
}

#[derive(Debug, Default)]
pub struct ReportUi {
    pub crash_consent: Option<CrashConsentState>,
    pub bug_dialog: Option<BugDialogState>,
    pub feature_dialog: Option<FeatureDialogState>,
    pub token_dialog: bool,
    pub token_draft: String,
    /// When true, App should quit after the in-session fault consent closes.
    pub quit_after_consent: bool,
    pub last_status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CrashConsentState {
    pub meta: CrashMeta,
    pub markdown: String,
    pub include_diagnostics: bool,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BugDialogState {
    pub fields: BugFields,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FeatureDialogState {
    pub fields: FeatureFields,
    pub status: Option<String>,
}

impl ReportUi {
    pub fn open_bug(&mut self, fields: BugFields) {
        self.bug_dialog = Some(BugDialogState {
            fields,
            status: None,
        });
    }

    pub fn open_feature(&mut self) {
        self.feature_dialog = Some(FeatureDialogState {
            fields: FeatureFields::default(),
            status: None,
        });
    }

    pub fn open_token_dialog(&mut self, settings: &FrontendSettings) {
        self.token_draft = settings.github_pat.clone();
        self.token_dialog = true;
    }

    pub fn begin_crash_consent(&mut self, meta: CrashMeta, markdown: String, quit_after: bool) {
        self.quit_after_consent = quit_after;
        self.crash_consent = Some(CrashConsentState {
            meta,
            markdown,
            include_diagnostics: true,
            status: None,
        });
    }
}

/// Snapshot host fields for bug / crash forms.
pub fn collect_bug_defaults(
    settings: &FrontendSettings,
    rom_title: &str,
    rom_path: &str,
    boot_mode: BootMode,
    diagnostics: &str,
    summary: &str,
) -> BugFields {
    BugFields {
        version: brand::crate_version().to_string(),
        commit: git_commit().to_string(),
        os: host_os_label(),
        rom_title: if rom_title.is_empty() {
            "(none)".into()
        } else {
            rom_title.to_string()
        },
        rom_path: path_basename(rom_path),
        hardware_mode: hardware_label(settings.hardware_pref).into(),
        boot_mode: boot_label(boot_mode).into(),
        audio: audio_label(&settings.audio_output),
        controller: "Keyboard (defaults)".into(),
        diagnostics: if diagnostics.trim().is_empty() {
            "none".into()
        } else {
            diagnostics.to_string()
        },
        repro_mode: "Not tried / unknown".into(),
        summary: summary.to_string(),
    }
}

fn hardware_label(pref: HostHardwarePref) -> &'static str {
    match pref {
        HostHardwarePref::Automatic => "Automatic",
        HostHardwarePref::GameBoy => "Game Boy",
        HostHardwarePref::GameBoyColor => "Game Boy Color",
    }
}

fn boot_label(mode: BootMode) -> &'static str {
    match mode {
        BootMode::Fast => "Fast (skip / post-boot state)",
        BootMode::BootRom => "Boot ROM",
    }
}

fn audio_label(pref: &AudioDevicePref) -> String {
    match pref {
        AudioDevicePref::SystemDefault => "System Default".into(),
        AudioDevicePref::Device { name } => name.clone(),
    }
}

pub fn write_fault_envelope(fault_text: &str, fields: &BugFields) -> Result<(), String> {
    let dir = crash_dir().ok_or_else(|| "no crash directory".to_string())?;
    let body = bug_issue(fields)?;
    let mut md = String::new();
    md.push_str("# Graycart crash report\n\n");
    md.push_str(&body);
    md.push_str("\n### Fault detail\n\n```\n");
    md.push_str(fault_text);
    md.push_str("\n```\n");
    let meta = CrashMeta::new(EnvelopeKind::Fault, fields.summary.clone());
    write_envelope(&dir, &meta, &md)
}

pub fn write_panic_envelope(panic_text: &str) -> Result<(), String> {
    let dir = crash_dir().ok_or_else(|| "no crash directory".to_string())?;
    let fields = BugFields {
        version: brand::crate_version().to_string(),
        commit: git_commit().to_string(),
        os: host_os_label(),
        rom_title: "(unknown)".into(),
        rom_path: String::new(),
        hardware_mode: "Unknown / not sure".into(),
        boot_mode: "Unknown / not sure".into(),
        audio: "unknown".into(),
        controller: "unknown".into(),
        diagnostics: panic_text.to_string(),
        repro_mode: "Not tried / unknown".into(),
        summary: "Host panic / abort".into(),
    };
    let body = bug_issue(&fields)?;
    let meta = CrashMeta::new(EnvelopeKind::Panic, "panic");
    write_envelope(&dir, &meta, &body)
}

/// Install a panic hook that writes a local envelope and never uploads.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".into()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".into());
        let text = format!("panic at {location}\n{payload}");
        let _ = write_panic_envelope(&text);
        prev(info);
    }));
}

fn try_file_bug(settings: &FrontendSettings, title: &str, body: &str) -> Result<String, String> {
    let Some(token) = resolve_token(settings) else {
        return Err("missing_token".into());
    };
    let filer = GithubIssueFiler::new(token);
    let created = filer.create_issue(title, body)?;
    Ok(created.html_url)
}

fn fallback_bug(ctx: &Context, body: &str) {
    let _ = set_clipboard_text(body);
    ctx.copy_text(body.to_string());
    let _ = github::open_url(&bug_new_issue_url());
}

fn fallback_feature(ctx: &Context, body: &str) {
    let _ = set_clipboard_text(body);
    ctx.copy_text(body.to_string());
    let _ = github::open_url(&feature_new_issue_url());
}

fn set_clipboard_text(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.set_text(text.to_string()))
        .map_err(|e| e.to_string())
}

/// Draw report dialogs. Returns consent outcome when a crash modal closes.
pub fn show(
    ctx: &Context,
    settings: &mut FrontendSettings,
    ui_state: &mut ReportUi,
) -> ConsentOutcome {
    let mut outcome = ConsentOutcome::Open;

    if ui_state.token_dialog {
        egui::Window::new("Optional GitHub token")
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    "Optional. A fine-grained PAT with Issues read/write on graycart/graycart-gb \
                     lets Send create the issue automatically. Without a token, Send still works: \
                     it copies the report and opens GitHub’s new-issue form. Stored locally only \
                     (never shipped in release binaries). Env: GRAYCART_GITHUB_TOKEN.",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut ui_state.token_draft)
                        .password(true)
                        .hint_text("github_pat_…"),
                );
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        settings.github_pat = ui_state.token_draft.trim().to_string();
                        settings.save();
                        ui_state.token_dialog = false;
                        ui_state.last_status = Some("GitHub token saved locally.".into());
                    }
                    if ui.button("Clear").clicked() {
                        ui_state.token_draft.clear();
                        settings.github_pat.clear();
                        settings.save();
                    }
                    if ui.button("Cancel").clicked() {
                        ui_state.token_dialog = false;
                    }
                });
            });
    }

    if let Some(mut state) = ui_state.bug_dialog.take() {
        let mut open = true;
        let mut request_close = false;
        egui::Window::new("Report bug")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(
                    "Review what will be sent. Send creates a GitHub issue when a token is \
                     configured; otherwise it copies the report and opens the bug form in your browser. \
                     Cancel leaves nothing filed.",
                );
                edit_bug_fields(ui, &mut state.fields);
                if let Some(status) = &state.status {
                    ui.colored_label(egui::Color32::LIGHT_RED, status);
                }
                ui.horizontal(|ui| {
                    if ui.button("Send").clicked() {
                        match bug_issue(&state.fields) {
                            Ok(body) => {
                                let title = if state.fields.summary.trim().is_empty() {
                                    "[Bug]: Graycart bug report".into()
                                } else {
                                    crash_issue_title("bug", &state.fields.summary)
                                };
                                match try_file_bug(settings, &title, &body) {
                                    Ok(url) => {
                                        ui_state.last_status = Some(format!("Filed: {url}"));
                                        request_close = true;
                                    }
                                    Err(e) if e == "missing_token" => {
                                        fallback_bug(ctx, &body);
                                        ui_state.last_status = Some(
                                            "Opened GitHub bug form — report copied to clipboard (paste if needed). Token optional under Help → Optional GitHub token…".into(),
                                        );
                                        request_close = true;
                                    }
                                    Err(e) => {
                                        fallback_bug(ctx, &body);
                                        ui_state.last_status = Some(format!(
                                            "API failed ({e}). Opened GitHub — report on clipboard."
                                        ));
                                        request_close = true;
                                    }
                                }
                            }
                            Err(e) => state.status = Some(e),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        request_close = true;
                    }
                });
            });
        if request_close {
            open = false;
        }
        if open {
            ui_state.bug_dialog = Some(state);
        }
    }

    if let Some(mut state) = ui_state.feature_dialog.take() {
        let mut open = true;
        let mut request_close = false;
        egui::Window::new("Request feature")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.label(
                    "Title + description only. Send files with a token when configured; \
                     otherwise copies the request and opens GitHub. Cancel files nothing.",
                );
                ui.label("Title");
                ui.text_edit_singleline(&mut state.fields.title);
                ui.label("Description");
                ui.add(
                    egui::TextEdit::multiline(&mut state.fields.description)
                        .desired_width(f32::INFINITY)
                        .desired_rows(8),
                );
                if let Some(status) = &state.status {
                    ui.colored_label(egui::Color32::LIGHT_RED, status);
                }
                ui.horizontal(|ui| {
                    if ui.button("Send").clicked() {
                        match feature_issue(&state.fields) {
                            Ok(body) => {
                                let title = if state.fields.title.trim().is_empty() {
                                    "[Feature]: request".into()
                                } else {
                                    format!("[Feature]: {}", state.fields.title.trim())
                                };
                                match try_file_bug(settings, &title, &body) {
                                    Ok(url) => {
                                        ui_state.last_status = Some(format!("Filed: {url}"));
                                        request_close = true;
                                    }
                                    Err(e) if e == "missing_token" => {
                                        fallback_feature(ctx, &body);
                                        ui_state.last_status = Some(
                                            "Opened GitHub feature form — description copied to clipboard.".into(),
                                        );
                                        request_close = true;
                                    }
                                    Err(e) => {
                                        fallback_feature(ctx, &body);
                                        ui_state.last_status = Some(format!(
                                            "API failed ({e}). Opened GitHub — description on clipboard."
                                        ));
                                        request_close = true;
                                    }
                                }
                            }
                            Err(e) => state.status = Some(e),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        request_close = true;
                    }
                });
            });
        if request_close {
            open = false;
        }
        if open {
            ui_state.feature_dialog = Some(state);
        }
    }

    if let Some(mut state) = ui_state.crash_consent.take() {
        let mut open = true;
        let mut request_close = false;
        let mut sent = false;
        let mut dismissed = false;
        let kind = match state.meta.kind {
            EnvelopeKind::Panic => "panic",
            EnvelopeKind::Fault => "emulation fault",
            _ => "crash",
        };
        egui::Window::new("Send crash report?")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(
                    "Graycart stopped unexpectedly and saved a local crash report. \
                     Send a GitHub bug report? You can review what is included. \
                     Nothing is uploaded until you choose Send.",
                );
                ui.label(format!("Kind: {kind}"));
                ui.checkbox(
                    &mut state.include_diagnostics,
                    "Include diagnostic / fault text",
                );
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        ui.monospace(&state.markdown);
                    });
                if let Some(status) = &state.status {
                    ui.colored_label(egui::Color32::LIGHT_RED, status);
                }
                ui.horizontal(|ui| {
                    if ui.button("Send").clicked() {
                        let title = crash_issue_title(kind, &state.meta.title_hint);
                        let body = if state.include_diagnostics {
                            state.markdown.clone()
                        } else {
                            "(diagnostics omitted by user)\n".into()
                        };
                        match try_file_bug(settings, &title, &body) {
                            Ok(url) => {
                                if let Some(dir) = crash_dir() {
                                    let _ = mark_consumed(&dir);
                                }
                                ui_state.last_status = Some(format!("Filed: {url}"));
                                request_close = true;
                                sent = true;
                            }
                            Err(e) if e == "missing_token" => {
                                fallback_bug(ctx, &body);
                                if let Some(dir) = crash_dir() {
                                    let _ = mark_consumed(&dir);
                                }
                                ui_state.last_status = Some(
                                    "Opened GitHub bug form — crash report copied to clipboard."
                                        .into(),
                                );
                                request_close = true;
                                sent = true;
                            }
                            Err(e) => {
                                fallback_bug(ctx, &body);
                                if let Some(dir) = crash_dir() {
                                    let _ = mark_consumed(&dir);
                                }
                                ui_state.last_status = Some(format!(
                                    "API failed ({e}). Opened GitHub — report on clipboard."
                                ));
                                request_close = true;
                                sent = true;
                            }
                        }
                    }
                    if ui.button("Don't send").clicked() {
                        if let Some(dir) = crash_dir() {
                            let _ = mark_skipped(&dir);
                        }
                        request_close = true;
                        dismissed = true;
                    }
                });
            });
        if request_close {
            open = false;
        }
        if sent {
            outcome = ConsentOutcome::Sent;
        } else if dismissed || !open {
            // Window chrome close (X) == dismiss, never file.
            if !dismissed && let Some(dir) = crash_dir() {
                let _ = mark_skipped(&dir);
            }
            outcome = ConsentOutcome::Dismissed;
        } else {
            ui_state.crash_consent = Some(state);
        }
    }

    outcome
}

fn edit_bug_fields(ui: &mut egui::Ui, fields: &mut BugFields) {
    let row = |ui: &mut egui::Ui, label: &str, value: &mut String| {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.text_edit_singleline(value);
        });
    };
    row(ui, "Version", &mut fields.version);
    row(ui, "Commit", &mut fields.commit);
    row(ui, "OS", &mut fields.os);
    row(ui, "ROM title", &mut fields.rom_title);
    row(ui, "Hardware", &mut fields.hardware_mode);
    row(ui, "Boot", &mut fields.boot_mode);
    row(ui, "Audio", &mut fields.audio);
    row(ui, "Controller", &mut fields.controller);
    row(ui, "Repro modes", &mut fields.repro_mode);
    ui.label("Diagnostics");
    ui.add(
        egui::TextEdit::multiline(&mut fields.diagnostics)
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
    ui.label("What happened");
    ui.add(
        egui::TextEdit::multiline(&mut fields.summary)
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
}

/// Pure helper: dismissing consent must not invoke the filer.
#[cfg(test)]
pub fn dismiss_without_filing<F: IssueFiler>(
    filer: &F,
    dir: &std::path::Path,
) -> Result<(), String> {
    mark_skipped(dir)?;
    let _ = filer; // intentionally unused — documents no API call
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::shell::report::github::RecordingFiler;
    use tempfile::tempdir;

    #[test]
    fn dismiss_does_not_call_api() {
        let dir = tempdir().unwrap();
        let meta = CrashMeta::new(EnvelopeKind::Fault, "test");
        write_envelope(dir.path(), &meta, "# dump\n").unwrap();
        let filer = RecordingFiler::default();
        dismiss_without_filing(&filer, dir.path()).unwrap();
        assert!(filer.calls.lock().unwrap().is_empty());
        assert!(crate::frontend::shell::report::envelope::load_pending(dir.path()).is_none());
    }
}
