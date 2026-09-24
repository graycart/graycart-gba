//! In-memory rewind rings (SM83 GCS state or ARM machine state).

use super::settings::FrontendSettings;
use crate::hw::ArmMachineState;
use graycart::MachineStateV1;

pub const REWIND_SOFT_CAP_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum RewindFrame {
    Sm83(MachineStateV1),
    Arm(ArmMachineState),
}

impl RewindFrame {
    pub fn heap_bytes(&self) -> usize {
        match self {
            Self::Sm83(s) => s.heap_bytes(),
            // Work RAM + VRAM + I/O + palette + OAM (ROM omitted from snapshots).
            Self::Arm(_) => 256 * 1024 + 32 * 1024 + 96 * 1024 + 2 * 1024 + 1024,
        }
    }
}

pub struct RewindRing {
    slots: Vec<RewindFrame>,
    head: usize,
    len: usize,
    bytes: usize,
    scrub_idx: Option<usize>,
}

impl RewindRing {
    pub fn new(template: RewindFrame) -> Self {
        Self {
            slots: vec![template],
            head: 0,
            len: 0,
            bytes: 0,
            scrub_idx: None,
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
        self.bytes = 0;
        self.scrub_idx = None;
    }

    pub fn push(&mut self, state: RewindFrame) {
        let cost = state.heap_bytes();
        while self.bytes.saturating_add(cost) > REWIND_SOFT_CAP_BYTES && self.len > 0 {
            self.pop_oldest();
        }
        if self.slots.len() <= self.len {
            self.slots.push(state.clone());
        }
        let cap = self.slots.len().max(1);
        let idx = (self.head + self.len) % cap;
        self.slots[idx] = state;
        self.len += 1;
        self.bytes += cost;
        self.scrub_idx = None;
    }

    pub fn begin_scrub(&mut self) {
        if self.scrub_idx.is_none() && self.len > 0 {
            let cap = self.slots.len();
            self.scrub_idx = Some((self.head + self.len - 1) % cap);
        }
    }

    pub fn end_scrub(&mut self) {
        let Some(scrub_idx) = self.scrub_idx.take() else {
            return;
        };
        if self.len == 0 {
            return;
        }
        let cap = self.slots.len();
        let new_len = (scrub_idx + cap - self.head) % cap + 1;
        if new_len >= self.len {
            return;
        }
        for pos in new_len..self.len {
            let idx = (self.head + pos) % cap;
            let cost = self.slots[idx].heap_bytes();
            self.bytes = self.bytes.saturating_sub(cost);
        }
        self.len = new_len;
    }

    pub fn scrub_back(&mut self) -> Option<&RewindFrame> {
        let idx = self.scrub_idx?;
        if self.len <= 1 {
            return None;
        }
        let cap = self.slots.len();
        if idx == self.head {
            return None;
        }
        let prev = (idx + cap - 1) % cap;
        self.scrub_idx = Some(prev);
        Some(&self.slots[prev])
    }

    fn pop_oldest(&mut self) {
        if self.len == 0 {
            return;
        }
        let cost = self.slots[self.head].heap_bytes();
        self.bytes = self.bytes.saturating_sub(cost);
        let cap = self.slots.len();
        self.head = (self.head + 1) % cap;
        self.len -= 1;
    }

    #[cfg(test)]
    fn oldest_pc(&self) -> u16 {
        if self.len == 0 {
            0
        } else {
            match &self.slots[self.head] {
                RewindFrame::Sm83(s) => s.cpu.pc,
                RewindFrame::Arm(s) => s.cpu.exec_pc as u16,
            }
        }
    }

    #[cfg(test)]
    fn newest_pc(&self) -> u16 {
        if self.len == 0 {
            0
        } else {
            let cap = self.slots.len();
            match &self.slots[(self.head + self.len - 1) % cap] {
                RewindFrame::Sm83(s) => s.cpu.pc,
                RewindFrame::Arm(s) => s.cpu.exec_pc as u16,
            }
        }
    }
}

#[allow(dead_code)]
pub fn enabled(settings: &FrontendSettings) -> bool {
    settings.rewind_enabled
}

#[cfg(test)]
mod tests;
