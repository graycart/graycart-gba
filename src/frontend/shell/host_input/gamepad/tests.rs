use super::{initialize_preferred_profile, newest_remaining, profile_key};
use crate::frontend::shell::host_input::InputSettings;
use std::collections::VecDeque;

#[test]
fn zero_uuid_falls_back_to_vid_pid_name() {
    assert_eq!(
        profile_key([0; 16], Some(0x057e), Some(0x2009), "Pro Controller"),
        "057e:2009:Pro Controller"
    );
}

#[test]
fn zero_uuid_uses_zero_for_missing_vendor_and_product() {
    assert_eq!(profile_key([0; 16], None, None, "Pad"), "0000:0000:Pad");
}

#[test]
fn non_zero_uuid_uses_lowercase_hex() {
    let mut uuid = [0; 16];
    uuid[0] = 0xab;
    uuid[15] = 0xcd;

    assert_eq!(
        profile_key(uuid, None, None, "ignored"),
        "ab0000000000000000000000000000cd"
    );
}

#[test]
fn disconnecting_active_uses_newest_remaining_session() {
    let preferred = Some("preferred".to_string());
    let remaining = VecDeque::from([2_u32, 3_u32]);

    assert_eq!(newest_remaining(&remaining), Some(3));
    assert_eq!(preferred.as_deref(), Some("preferred"));
}

#[test]
fn disconnecting_shared_profile_selects_remaining_pad() {
    let preferred = Some("shared".to_string());
    let remaining = VecDeque::from([("shared", 1_u32), ("shared", 2_u32)]);

    assert_eq!(newest_remaining(&remaining), Some(("shared", 2)));
    assert_eq!(preferred.as_deref(), Some("shared"));
}

#[test]
fn disconnect_resolution_returns_none_without_remaining_pads() {
    assert_eq!(newest_remaining::<u32>(&VecDeque::new()), None);
}

#[test]
fn initializing_first_preference_reports_settings_dirty() {
    let mut settings = InputSettings::default();

    assert!(initialize_preferred_profile(&mut settings, "shared"));
    assert_eq!(
        settings.preferred_controller_profile_id.as_deref(),
        Some("shared")
    );
}

#[test]
fn existing_preference_is_not_dirty_or_replaced() {
    let mut settings = InputSettings {
        preferred_controller_profile_id: Some("preferred".into()),
        ..InputSettings::default()
    };

    assert!(!initialize_preferred_profile(&mut settings, "other"));
    assert_eq!(
        settings.preferred_controller_profile_id.as_deref(),
        Some("preferred")
    );
}
