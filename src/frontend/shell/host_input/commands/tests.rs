use super::*;
use graycart::GameBoyButton;
use winit::keyboard::KeyCode;

#[test]
fn host_command_defaults() {
    let map = HostCommandMap::default();
    assert_eq!(map.key(HostCommand::QuickSave), KeyCode::F5);
    assert_eq!(map.key(HostCommand::QuickLoad), KeyCode::F8);
    assert_eq!(map.key(HostCommand::Screenshot), KeyCode::F6);
    assert_eq!(map.key(HostCommand::Rewind), KeyCode::KeyR);
    assert_eq!(map.key(HostCommand::Fullscreen), KeyCode::F11);
    assert_eq!(map.key(HostCommand::Monitor), KeyCode::F12);
}

#[test]
fn host_command_conflict_detects_overlap_with_keyboard_map() {
    let host = HostCommandMap::default();
    let mut kb = KeyboardMap::default();
    kb.bind(GameBoyButton::A, KeyCode::F5);
    let conflicts = host.conflicts_with(&kb);
    assert!(conflicts.iter().any(|(c, _)| *c == HostCommand::QuickSave));
}

#[test]
fn host_command_bind_swaps_on_conflict() {
    let mut map = HostCommandMap::default();
    map.bind(HostCommand::QuickSave, KeyCode::F8);
    assert_eq!(map.key(HostCommand::QuickSave), KeyCode::F8);
    assert_eq!(map.key(HostCommand::QuickLoad), KeyCode::F5);
}

#[test]
fn is_reserved_host_key_matches_defaults() {
    let host = HostCommandMap::default();
    assert!(is_reserved_host_key(KeyCode::F5, &host));
    assert!(!is_reserved_host_key(KeyCode::KeyZ, &host));
}

#[test]
fn keyed_round_trip_includes_pause_and_ff() {
    let mut map = HostCommandMap::default();
    map.bind(HostCommand::QuickSave, KeyCode::Digit1);
    map.bind(HostCommand::Pause, KeyCode::KeyP);
    map.bind(HostCommand::FrameAdvance, KeyCode::KeyF);
    map.bind(HostCommand::FastForwardHold, KeyCode::KeyH);
    map.bind(HostCommand::ToggleFastForward, KeyCode::KeyT);

    let json = serde_json::to_string(&map).unwrap();
    assert!(json.contains("\"pause\":"));
    assert!(json.contains("\"frame_advance\":"));
    assert!(json.contains("\"fast_forward_hold\":"));
    assert!(json.contains("\"toggle_fast_forward\":"));

    let loaded: HostCommandMap = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded, map);
}

#[test]
fn migrates_legacy_positional_six_array() {
    let json = r#"["Digit1","Digit2","Digit3","Digit4","Digit5","Digit6"]"#;
    let map: HostCommandMap = serde_json::from_str(json).unwrap();
    assert_eq!(map.key(HostCommand::QuickSave), KeyCode::Digit1);
    assert_eq!(map.key(HostCommand::QuickLoad), KeyCode::Digit2);
    assert_eq!(map.key(HostCommand::Screenshot), KeyCode::Digit3);
    assert_eq!(map.key(HostCommand::Rewind), KeyCode::Digit4);
    assert_eq!(map.key(HostCommand::Fullscreen), KeyCode::Digit5);
    assert_eq!(map.key(HostCommand::Monitor), KeyCode::Digit6);
    assert_eq!(map.key(HostCommand::Pause), KeyCode::Space);
    assert_eq!(map.key(HostCommand::FrameAdvance), KeyCode::Period);
    assert_eq!(map.key(HostCommand::FastForwardHold), KeyCode::Tab);
    assert_eq!(map.key(HostCommand::ToggleFastForward), KeyCode::Backslash);
}

#[test]
fn defaults_space_period_tab_backslash() {
    let map = HostCommandMap::default();
    assert_eq!(map.key(HostCommand::Pause), KeyCode::Space);
    assert_eq!(map.key(HostCommand::FrameAdvance), KeyCode::Period);
    assert_eq!(map.key(HostCommand::FastForwardHold), KeyCode::Tab);
    assert_eq!(map.key(HostCommand::ToggleFastForward), KeyCode::Backslash);
}
