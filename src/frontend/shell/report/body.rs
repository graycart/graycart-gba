//! Markdown issue bodies aligned with Stream B issue-form field ids.

use super::scrub::{looks_like_rom_bytes, path_basename};

/// Bug metadata mirroring `.github/ISSUE_TEMPLATE/bug.yml` ids.
#[derive(Debug, Clone, Default)]
pub struct BugFields {
    pub version: String,
    pub commit: String,
    pub os: String,
    pub rom_title: String,
    /// Scrubbed path (basename only) for diagnostics context.
    pub rom_path: String,
    pub hardware_mode: String,
    pub boot_mode: String,
    pub audio: String,
    pub controller: String,
    pub diagnostics: String,
    /// Free-form checklist text for `repro_mode` (e.g. "Automatic, Game Boy (DMG)").
    pub repro_mode: String,
    pub summary: String,
}

#[derive(Debug, Clone, Default)]
pub struct FeatureFields {
    pub title: String,
    pub description: String,
}

fn field(label: &str, value: &str) -> String {
    format!("### {label}\n\n{}\n", value.trim())
}

/// Build a bug-report markdown body using Stream B field labels/ids.
pub fn bug_issue(fields: &BugFields) -> Result<String, String> {
    if looks_like_rom_bytes(&fields.diagnostics) || looks_like_rom_bytes(&fields.summary) {
        return Err("diagnostics must not embed ROM bytes".into());
    }
    let rom_path = path_basename(&fields.rom_path);
    let mut out = String::new();
    out.push_str(&field("Graycart version", &fields.version));
    out.push_str(&field("Commit / SHA", &fields.commit));
    out.push_str(&field("OS", &fields.os));
    out.push_str(&field("ROM title / header", &fields.rom_title));
    if !rom_path.is_empty() {
        out.push_str(&field("ROM basename (scrubbed)", &rom_path));
    }
    out.push_str(&field("Hardware mode", &fields.hardware_mode));
    out.push_str(&field("Boot mode", &fields.boot_mode));
    out.push_str(&field("Audio backend / device", &fields.audio));
    out.push_str(&field("Controller / input", &fields.controller));
    out.push_str(&field("Diagnostic capture", &fields.diagnostics));
    out.push_str(&field("Reproduces on", &fields.repro_mode));
    out.push_str(&field("What happened", &fields.summary));
    Ok(out)
}

pub fn feature_issue(fields: &FeatureFields) -> Result<String, String> {
    if fields.description.trim().is_empty() {
        return Err("description is required".into());
    }
    if looks_like_rom_bytes(&fields.description) {
        return Err("description must not embed ROM bytes".into());
    }
    Ok(field("Description", &fields.description))
}

pub fn crash_issue_title(kind_label: &str, hint: &str) -> String {
    let hint = hint.trim();
    if hint.is_empty() {
        format!("[Bug]: {kind_label}")
    } else {
        let short: String = hint.chars().take(72).collect();
        format!("[Bug]: {kind_label} — {short}")
    }
}

pub fn host_os_label() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

pub fn git_commit() -> &'static str {
    option_env!("GRAYCART_GIT_SHA").unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bug_body_includes_stream_b_field_labels() {
        let body = bug_issue(&BugFields {
            version: "0.11.1".into(),
            commit: "abc".into(),
            os: "linux x86_64".into(),
            rom_title: "TEST".into(),
            rom_path: "/home/u/secret/cart.gb".into(),
            hardware_mode: "Automatic".into(),
            boot_mode: "Fast (skip / post-boot state)".into(),
            audio: "System Default".into(),
            controller: "Keyboard (defaults)".into(),
            diagnostics: "none".into(),
            repro_mode: "Automatic".into(),
            summary: "steps…".into(),
        })
        .unwrap();
        assert!(body.contains("### Graycart version"));
        assert!(body.contains("### Commit / SHA"));
        assert!(body.contains("### ROM title / header"));
        assert!(body.contains("cart.gb"));
        assert!(!body.contains("/home/u/secret"));
        assert!(body.contains("### What happened"));
    }

    #[test]
    fn feature_body_is_description_only() {
        let body = feature_issue(&FeatureFields {
            title: "Save states cloud".into(),
            description: "I want cloud saves".into(),
        })
        .unwrap();
        assert!(body.contains("### Description"));
        assert!(body.contains("I want cloud saves"));
        assert!(!body.contains("Graycart version"));
    }

    #[test]
    fn bug_rejects_rom_byte_diagnostics() {
        let err = bug_issue(&BugFields {
            diagnostics: "x\0\0\0y".into(),
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("ROM bytes"));
    }
}
