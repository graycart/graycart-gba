//! Resolve the user fine-grained PAT used for Issues API filing.

use super::super::settings::FrontendSettings;

pub const ENV_TOKEN: &str = "GRAYCART_GITHUB_TOKEN";

/// Resolution order: settings field → env `GRAYCART_GITHUB_TOKEN` → missing.
pub fn resolve_token(settings: &FrontendSettings) -> Option<String> {
    let from_settings = settings.github_pat.trim();
    if !from_settings.is_empty() {
        return Some(from_settings.to_string());
    }
    std::env::var(ENV_TOKEN)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Parallel cargo tests race on process-wide env; serialize mutations of ENV_TOKEN.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn settings_beats_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let settings = FrontendSettings {
            github_pat: "settings-token".into(),
            ..Default::default()
        };
        // SAFETY: test process owns env for this key; we restore after.
        let prev = std::env::var(ENV_TOKEN).ok();
        unsafe { std::env::set_var(ENV_TOKEN, "env-token") };
        assert_eq!(resolve_token(&settings).as_deref(), Some("settings-token"));
        match prev {
            Some(v) => unsafe { std::env::set_var(ENV_TOKEN, v) },
            None => unsafe { std::env::remove_var(ENV_TOKEN) },
        }
    }

    #[test]
    fn env_used_when_settings_blank() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let settings = FrontendSettings::default();
        let prev = std::env::var(ENV_TOKEN).ok();
        unsafe { std::env::set_var(ENV_TOKEN, "  env-only  ") };
        assert_eq!(resolve_token(&settings).as_deref(), Some("env-only"));
        match prev {
            Some(v) => unsafe { std::env::set_var(ENV_TOKEN, v) },
            None => unsafe { std::env::remove_var(ENV_TOKEN) },
        }
    }
}
