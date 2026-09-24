use super::{
    SpeedPreset, StepPulse, audio_transition_needs_flush, clear_transient_holds, effective_speed,
    run_until_frame_or_budget, should_submit_audio,
};

#[test]
fn multiplier_maps_finite_presets() {
    assert_eq!(SpeedPreset::X1.multiplier(), Some(1));
    assert_eq!(SpeedPreset::X2.multiplier(), Some(2));
    assert_eq!(SpeedPreset::X3.multiplier(), Some(3));
    assert_eq!(SpeedPreset::X4.multiplier(), Some(4));
    assert_eq!(SpeedPreset::X8.multiplier(), Some(8));
    assert_eq!(SpeedPreset::Unlimited.multiplier(), None);
}

#[test]
fn labels_match_overlay_spec() {
    assert_eq!(SpeedPreset::X1.label(), "1×");
    assert_eq!(SpeedPreset::X2.label(), "2×");
    assert_eq!(SpeedPreset::X3.label(), "3×");
    assert_eq!(SpeedPreset::X4.label(), "4×");
    assert_eq!(SpeedPreset::X8.label(), "8×");
    assert_eq!(SpeedPreset::Unlimited.label(), "MAX");
}

#[test]
fn serde_round_trip_all_variants() {
    for preset in SpeedPreset::ALL {
        let json = serde_json::to_string(&preset).unwrap();
        let loaded: SpeedPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, preset);
    }
}

#[test]
fn effective_speed_idle_is_x1() {
    assert_eq!(
        effective_speed(false, false, false, false, SpeedPreset::X4),
        SpeedPreset::X1
    );
}

#[test]
fn effective_speed_hold_or_toggle_uses_target() {
    for target in SpeedPreset::ALL {
        assert_eq!(
            effective_speed(false, false, true, false, target),
            target,
            "hold"
        );
        assert_eq!(
            effective_speed(false, false, false, true, target),
            target,
            "toggle"
        );
    }
}

#[test]
fn effective_speed_pause_and_rewind_force_x1_for_pacing() {
    let target = SpeedPreset::X8;
    assert_eq!(
        effective_speed(true, false, true, true, target),
        SpeedPreset::X1
    );
    assert_eq!(
        effective_speed(false, true, true, true, target),
        SpeedPreset::X1
    );
    assert_eq!(
        effective_speed(true, true, true, true, target),
        SpeedPreset::X1
    );
}

#[test]
fn clear_transient_holds_clears_both_and_reports() {
    let mut ff = true;
    let mut rw = true;
    assert!(clear_transient_holds(&mut ff, &mut rw));
    assert!(!ff);
    assert!(!rw);
}

#[test]
fn clear_transient_holds_false_when_nothing_active() {
    let mut ff = false;
    let mut rw = false;
    assert!(!clear_transient_holds(&mut ff, &mut rw));
}

#[test]
fn clear_transient_holds_partial() {
    let mut ff = true;
    let mut rw = false;
    assert!(clear_transient_holds(&mut ff, &mut rw));
    assert!(!ff);
    assert!(!rw);
}

#[test]
fn run_until_frame_or_budget_returns_frame_ready() {
    use std::cell::Cell;

    let steps = Cell::new(0u32);
    let mut step_once = || {
        steps.set(steps.get() + 1);
        steps.get() >= 3
    };

    assert_eq!(
        run_until_frame_or_budget(&mut step_once, 10),
        StepPulse::FrameReady
    );
    assert_eq!(steps.get(), 3);
}

#[test]
fn run_until_frame_or_budget_exhausts_budget() {
    let mut steps = 0u32;
    let mut step_once = || {
        steps += 1;
        false
    };

    assert_eq!(
        run_until_frame_or_budget(&mut step_once, 5),
        StepPulse::BudgetExhausted
    );
    assert_eq!(steps, 5);
}

#[test]
fn run_until_frame_or_budget_zero_budget() {
    let mut steps = 0u32;
    let mut step_once = || {
        steps += 1;
        true
    };

    assert_eq!(
        run_until_frame_or_budget(&mut step_once, 0),
        StepPulse::BudgetExhausted
    );
    assert_eq!(steps, 0);
}

#[test]
fn should_submit_audio_only_at_x1_play() {
    assert!(should_submit_audio(SpeedPreset::X1, false, false));
    assert!(!should_submit_audio(SpeedPreset::X4, false, false));
    assert!(!should_submit_audio(SpeedPreset::Unlimited, false, false));
    assert!(!should_submit_audio(SpeedPreset::X1, true, false));
    assert!(!should_submit_audio(SpeedPreset::X1, false, true));
}

#[test]
fn audio_transition_needs_flush_on_turbo_and_rewind_edges() {
    assert!(audio_transition_needs_flush(
        SpeedPreset::X1,
        false,
        SpeedPreset::X4,
        false,
    ));
    assert!(audio_transition_needs_flush(
        SpeedPreset::X4,
        false,
        SpeedPreset::X1,
        false,
    ));
    assert!(audio_transition_needs_flush(
        SpeedPreset::X1,
        false,
        SpeedPreset::X1,
        true,
    ));
    assert!(audio_transition_needs_flush(
        SpeedPreset::X1,
        true,
        SpeedPreset::X1,
        false,
    ));
    assert!(!audio_transition_needs_flush(
        SpeedPreset::X4,
        false,
        SpeedPreset::X8,
        false,
    ));
    assert!(!audio_transition_needs_flush(
        SpeedPreset::X1,
        false,
        SpeedPreset::X1,
        false,
    ));
}
