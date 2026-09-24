//! Emulation thread loop — sole owner of [`Machine`](machine::Machine).

mod handle;
mod machine;
mod state;
mod tick;

use super::super::host_input::HostCommand;
use super::super::launch::Verbosity;
use super::super::playback::{SpeedPreset, effective_speed};
use super::RuntimeConfig;
use super::commands::EmuCommand;
use super::frame::{LatestDebug, LatestFrame};
use state::EmuState;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

pub(super) const T_CYCLES_PER_FRAME: u64 = 70_224;
pub(super) const DEBUG_PUBLISH_INTERVAL: Duration = Duration::from_millis(66);
pub(super) const RUNTIME_DIAG_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) fn emu_loop(
    config: RuntimeConfig,
    cmd_rx: Receiver<EmuCommand>,
    latest: LatestFrame,
    latest_debug: LatestDebug,
) {
    let mut state = EmuState::new(config);
    let runtime_diag =
        env_flag("GRAYCART_RUNTIME_DIAG") || state.config.verbosity == Verbosity::Verbose;

    while !state.quit {
        if !drain_commands(&mut state, &cmd_rx) {
            break;
        }
        if state.quit {
            break;
        }

        if !state.ready {
            // Idle until Start: sleep + try_recv only (never block on presentation).
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        let mut packet = state.run_tick();
        packet.frame_publish_replaced = latest.replaced_count();
        latest.publish(packet);
        state.maybe_publish_debug(&latest_debug);
        if runtime_diag {
            state.maybe_log_runtime_diag(&latest);
        }

        if state.quit {
            break;
        }

        let ff_hold = state.host_down.contains(&HostCommand::FastForwardHold);
        let rewinding = state.rewind_setting && state.host_down.contains(&HostCommand::Rewind);
        let speed = if state.config.unthrottled {
            SpeedPreset::Unlimited
        } else {
            effective_speed(
                state.paused,
                rewinding,
                ff_hold,
                state.ff_toggle,
                state.ff_speed,
            )
        };
        let unlimited = state.config.unthrottled || speed == SpeedPreset::Unlimited;
        let skip_sleep = state.audio_emergency_catch_up(state.paused, unlimited, rewinding, speed);
        let wake = state
            .scheduler
            .wake(state.paused, unlimited, skip_sleep, Instant::now());
        if !state.sleep_for_wake(wake, &cmd_rx) {
            break;
        }
        if !drain_commands(&mut state, &cmd_rx) {
            break;
        }
    }

    if let Err(e) = state.flush_save() {
        let _ = e;
    }
}

fn env_flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => matches!(
            v.as_str(),
            "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
        ),
        Err(_) => false,
    }
}

fn wait_until(deadline: Instant, cmd_rx: &Receiver<EmuCommand>, state: &mut EmuState) -> bool {
    while Instant::now() < deadline {
        if !drain_commands(state, cmd_rx) {
            return false;
        }
        if state.quit {
            return true;
        }
        let rem = deadline.saturating_duration_since(Instant::now());
        if rem.is_zero() {
            break;
        }
        thread::sleep(rem.min(Duration::from_millis(1)));
    }
    true
}

/// Drain pending commands with `try_recv` only. Returns `false` on disconnect.
fn drain_commands(state: &mut EmuState, cmd_rx: &Receiver<EmuCommand>) -> bool {
    loop {
        match cmd_rx.try_recv() {
            Ok(cmd) => {
                state.handle_command(cmd);
                if state.quit {
                    return true;
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return false,
        }
    }
}
