//! BIOS Halt (SWI 2) → IRQ entry phase, in cycles since the SWI retired.
//!
//! The HLE keeps no BIOS image, so the SWI body is a timeline instead:
//!
//! ```text
//! age 0 ........ 33      34      35 ...................... wake
//!     SWI body    │  HALTCNT write │ core halted, wake allowed
//!     (IRQ taken at the boundary before the strb: LR = 0xB8)
//! ```
//!
//! * An IRQ asserted by [`WRITE_BOUNDARY`] is taken at that boundary, before the
//!   BIOS stores HALTCNT. The stacked link is the BIOS `strb` + 4 (`0xB8`).
//! * Later IRQs find the core halted (the write retires at 34, and the core cannot
//!   wake before [`SETTLE`]). The stacked link is `0x74`.
//! * Either way the BIOS prologue (`stmfd`, `ldr pc,[0x03007FFC]`) runs before the
//!   user handler's first instruction.
//!
//! Reference: alyosha `irq/halt_pc*.asm` and `Interactions/Halt_IRQ.asm`. The
//! timer reload in each test (FFA0 / FFDC / FFDD / FFDE) moves the IRQ along this
//! timeline; the handler reads the timer back at its fifth instruction.

/// Last SWI-body boundary at which a pending IRQ is taken (before HALTCNT).
pub(crate) const WRITE_BOUNDARY: u32 = 33;
/// Earliest age at which a halted core can wake.
pub(crate) const SETTLE: u32 = 35;
/// Cycles from the wake to the handler's first fetch (timer / serial sources).
const PROLOGUE_TIMER: u32 = 42;
/// Non-timer wakes (DISPSTAT / serial) reach the handler this many cycles sooner
/// than a timer overflow wake. alyosha `Halt_IRQ` and `serial_time*` use this;
/// `halt_pc*` keeps the full [`PROLOGUE_TIMER`].
const SHORT_WAKE_LEAD: u32 = 35;
/// Same as [`PROLOGUE_TIMER`], one cycle shorter when the IRQ lands in the SWI body.
const PROLOGUE_EARLY: u32 = 41;

/// Stacked link for a Halt interrupted in the SWI body.
const LR_EARLY: u32 = 0xB8;
/// Stacked link for a Halt interrupted while halted.
const LR_HALTED: u32 = 0x74;

/// Stacked LR and cycles still owed to the machine clock for an IRQ that wakes
/// Halt `age` cycles after the SWI. `via_swi` is false for a raw HALTCNT write,
/// which has no SWI body: it always takes the halted path. `short_wake` is true
/// when every waking `IE & IF` bit is DISPSTAT or serial (not a timer).
pub(crate) fn halt_entry(age: u32, via_swi: bool, short_wake: bool) -> (u32, u32) {
    let halted = if short_wake {
        PROLOGUE_TIMER - SHORT_WAKE_LEAD
    } else {
        PROLOGUE_TIMER
    };
    if !via_swi {
        return (LR_HALTED, halted);
    }
    if age <= WRITE_BOUNDARY {
        // Wait out the rest of the SWI body, then enter at the boundary.
        (LR_EARLY, (WRITE_BOUNDARY - age) + PROLOGUE_EARLY)
    } else {
        (LR_HALTED, SETTLE.saturating_sub(age) + halted)
    }
}
