//! Per-channel DMA register file and internal transfer latches.
//!
//! Cited: GBATEK -- GBA DMA Transfers
//!   https://problemkaputt.de/gbatek-gba-dma-transfers.htm
//! Note: programmer-visible SAD/DAD/CNT plus internal reload latches.
//! P5: Immediate / VBlank / HBlank / Special arming; Repeat finish; IRQ bit.

/// Destination address control (DMAxCNT_H bits 5–6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DestControl {
    Increment = 0,
    Decrement = 1,
    Fixed = 2,
    IncrementReload = 3,
}

impl DestControl {
    pub fn from_bits(bits: u16) -> Self {
        match bits & 3 {
            0 => Self::Increment,
            1 => Self::Decrement,
            2 => Self::Fixed,
            _ => Self::IncrementReload,
        }
    }
}

/// Source address control (DMAxCNT_H bits 7–8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SrcControl {
    Increment = 0,
    Decrement = 1,
    Fixed = 2,
    /// GBATEK: prohibited — latched as Fixed (no undefined walk).
    Prohibited = 3,
}

impl SrcControl {
    pub fn from_bits(bits: u16) -> Self {
        match bits & 3 {
            0 => Self::Increment,
            1 => Self::Decrement,
            2 => Self::Fixed,
            _ => Self::Prohibited,
        }
    }
}

/// DMA start timing (DMAxCNT_H bits 12–13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StartTiming {
    Immediate = 0,
    VBlank = 1,
    HBlank = 2,
    Special = 3,
}

impl StartTiming {
    pub fn from_bits(bits: u16) -> Self {
        match bits & 3 {
            0 => Self::Immediate,
            1 => Self::VBlank,
            2 => Self::HBlank,
            _ => Self::Special,
        }
    }
}

/// Channel index 0..=3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ChannelId {
    Ch0 = 0,
    Ch1 = 1,
    Ch2 = 2,
    Ch3 = 3,
}

impl ChannelId {
    pub const ALL: [ChannelId; 4] = [
        ChannelId::Ch0,
        ChannelId::Ch1,
        ChannelId::Ch2,
        ChannelId::Ch3,
    ];

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Word-count width: 14-bit (0–2) or 16-bit (3).
    #[inline]
    pub fn count_mask(self) -> u32 {
        if self == ChannelId::Ch3 {
            0xFFFF
        } else {
            0x3FFF
        }
    }

    /// Word count of 0 means max (`0x4000` / `0x10000`).
    #[inline]
    pub fn max_count(self) -> u32 {
        if self == ChannelId::Ch3 {
            0x1_0000
        } else {
            0x4000
        }
    }

    /// Address mask applied on write (GBATEK: low 27/28 bits).
    #[inline]
    pub fn addr_mask(self) -> u32 {
        if self == ChannelId::Ch3 {
            0x0FFF_FFFF
        } else {
            0x07FF_FFFF
        }
    }

    /// IE/IF bit for this channel's completion IRQ.
    #[inline]
    pub fn irq_bit(self) -> u16 {
        match self {
            ChannelId::Ch0 => crate::irq::IRQ_DMA0,
            ChannelId::Ch1 => crate::irq::IRQ_DMA1,
            ChannelId::Ch2 => crate::irq::IRQ_DMA2,
            ChannelId::Ch3 => crate::irq::IRQ_DMA3,
        }
    }
}

impl TryFrom<usize> for ChannelId {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChannelId::Ch0),
            1 => Ok(ChannelId::Ch1),
            2 => Ok(ChannelId::Ch2),
            3 => Ok(ChannelId::Ch3),
            _ => Err(()),
        }
    }
}

/// One DMA channel: visible regs + internal latches used while transferring.
#[derive(Debug, Clone)]
pub struct Channel {
    pub id: ChannelId,
    /// Programmer-visible source address (write-only on hardware).
    pub sad: u32,
    /// Programmer-visible destination address (write-only on hardware).
    pub dad: u32,
    /// Programmer-visible word count (masked width).
    pub count: u16,
    /// DMAxCNT_H control word.
    pub control: u16,

    /// Internal SAD latch (does not walk the visible reg).
    pub(crate) latched_sad: u32,
    /// Internal DAD latch.
    pub(crate) latched_dad: u32,
    /// Remaining unit count for the active / pending burst.
    pub(crate) remaining: u32,
    /// True while this channel owns the bus for a burst.
    pub(crate) active: bool,
    /// Armed Immediate (or edge-triggered) burst waiting to drain.
    ///
    /// Kept as `pending_immediate` name for P2 test compatibility; used for all
    /// start modes once an edge / Immediate arms the channel.
    pub(crate) pending_immediate: bool,
    /// Cycles remaining before an armed start becomes [`Self::pending_immediate`]
    /// (GBATEK: wait **2** cycles after Enable 0→1 / start request).
    pub(crate) startup_delay: u32,
}

impl Channel {
    pub fn new(id: ChannelId) -> Self {
        Self {
            id,
            sad: 0,
            dad: 0,
            count: 0,
            control: 0,
            latched_sad: 0,
            latched_dad: 0,
            remaining: 0,
            active: false,
            pending_immediate: false,
            startup_delay: 0,
        }
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.control & CONTROL_ENABLE != 0
    }

    #[inline]
    pub fn repeat(&self) -> bool {
        self.control & CONTROL_REPEAT != 0
    }

    #[inline]
    pub fn transfer32(&self) -> bool {
        self.control & CONTROL_TRANSFER_TYPE != 0
    }

    #[inline]
    pub fn irq_enable(&self) -> bool {
        self.control & CONTROL_IRQ != 0
    }

    #[inline]
    pub fn dest_control(&self) -> DestControl {
        DestControl::from_bits(self.control >> 5)
    }

    #[inline]
    pub fn src_control(&self) -> SrcControl {
        SrcControl::from_bits(self.control >> 7)
    }

    #[inline]
    pub fn start_timing(&self) -> StartTiming {
        StartTiming::from_bits(self.control >> 12)
    }

    #[inline]
    pub fn unit_bytes(&self) -> u32 {
        if self.transfer32() {
            4
        } else {
            2
        }
    }

    pub fn write_sad(&mut self, value: u32) {
        self.sad = value & self.id.addr_mask();
    }

    pub fn write_dad(&mut self, value: u32) {
        let mask = if self.id == ChannelId::Ch3 {
            0x0FFF_FFFF
        } else {
            0x07FF_FFFF
        };
        self.dad = value & mask;
    }

    pub fn write_count(&mut self, value: u16) {
        self.count = value & (self.id.count_mask() as u16);
    }

    /// Write DMAxCNT_H. Rising enable reloads latches; Immediate schedules a
    /// 2-cycle startup delay before pending (G8-dma-delay).
    ///
    /// Non-Immediate modes stay enabled until a start edge / FIFO / capture hook
    /// calls [`Self::request_start`].
    pub fn write_control(&mut self, value: u16) -> bool {
        let was_enabled = self.enabled();
        self.control = value;
        let now_enabled = self.enabled();
        let rising = !was_enabled && now_enabled;

        if rising {
            self.reload_latches_full();
            if self.start_timing() == StartTiming::Immediate {
                self.startup_delay = 2;
                self.pending_immediate = false;
                self.active = false;
                return true;
            }
            self.startup_delay = 0;
            self.pending_immediate = false;
            self.active = false;
            return false;
        }

        if !now_enabled {
            self.active = false;
            self.pending_immediate = false;
            self.startup_delay = 0;
            self.remaining = 0;
        }
        false
    }

    /// Reload SAD/DAD/CNT latches from visible regs (Enable 0→1).
    pub(crate) fn reload_latches_full(&mut self) {
        self.latched_sad = self.sad;
        self.latched_dad = self.dad;
        self.reload_count_latch();
    }

    /// Reload remaining from visible CNT (Repeat path).
    pub(crate) fn reload_count_latch(&mut self) {
        let raw = u32::from(self.count) & self.id.count_mask();
        self.remaining = if raw == 0 { self.id.max_count() } else { raw };
    }

    /// Edge / FIFO / capture: mark channel pending if enabled for `timing`.
    pub(crate) fn request_start(&mut self, timing: StartTiming) -> bool {
        if !self.enabled() || self.active || self.pending_immediate || self.startup_delay > 0 {
            return false;
        }
        if self.start_timing() != timing {
            return false;
        }
        // DMA0 Special is illegal (GBATEK) — never arm.
        if timing == StartTiming::Special && self.id == ChannelId::Ch0 {
            return false;
        }
        self.pending_immediate = true;
        true
    }

    /// FIFO Special: force 4×32-bit units; dest stays at latched DAD (fixed).
    pub(crate) fn arm_fifo_burst(&mut self) {
        self.remaining = 4;
        // Force 32-bit for the burst regardless of CNT_H size bit.
        self.control |= CONTROL_TRANSFER_TYPE;
        self.startup_delay = 0;
        self.pending_immediate = true;
        self.active = false;
    }

    /// Tick startup delay; when it reaches 0, arm [`Self::pending_immediate`].
    pub(crate) fn tick_startup(&mut self, cycles: u32) {
        if self.startup_delay == 0 || cycles == 0 {
            return;
        }
        if cycles >= self.startup_delay {
            self.startup_delay = 0;
            self.pending_immediate = true;
        } else {
            self.startup_delay -= cycles;
        }
    }

    pub(crate) fn begin_active(&mut self) {
        self.pending_immediate = false;
        self.startup_delay = 0;
        self.active = true;
    }

    /// Finish a burst. Returns whether CNT_H.IRQ should raise IF.
    ///
    /// Immediate always clears Enable (Repeat ignored). Other modes keep Enable
    /// and reload CNT (and DAD if Inc+Reload) when Repeat is set.
    pub(crate) fn finish(&mut self) -> bool {
        let want_irq = self.irq_enable();
        self.active = false;
        self.pending_immediate = false;
        self.startup_delay = 0;

        let immediate = self.start_timing() == StartTiming::Immediate;
        if immediate || !self.repeat() {
            self.control &= !CONTROL_ENABLE;
            self.remaining = 0;
        } else {
            self.reload_count_latch();
            if self.dest_control() == DestControl::IncrementReload {
                self.latched_dad = self.dad;
            }
        }
        want_irq
    }

    pub(crate) fn step_addrs(&mut self) {
        let step = self.unit_bytes();
        match self.src_control() {
            SrcControl::Increment => self.latched_sad = self.latched_sad.wrapping_add(step),
            SrcControl::Decrement => self.latched_sad = self.latched_sad.wrapping_sub(step),
            SrcControl::Fixed | SrcControl::Prohibited => {}
        }
        match self.dest_control() {
            DestControl::Increment | DestControl::IncrementReload => {
                self.latched_dad = self.latched_dad.wrapping_add(step);
            }
            DestControl::Decrement => {
                self.latched_dad = self.latched_dad.wrapping_sub(step);
            }
            DestControl::Fixed => {}
        }
    }
}

impl Default for Channel {
    fn default() -> Self {
        Self::new(ChannelId::Ch0)
    }
}

pub const CONTROL_REPEAT: u16 = 1 << 9;
pub const CONTROL_TRANSFER_TYPE: u16 = 1 << 10;
pub const CONTROL_IRQ: u16 = 1 << 14;
pub const CONTROL_ENABLE: u16 = 1 << 15;

/// Build a CNT_H value for tests / helpers.
pub fn control_word(
    dest: DestControl,
    src: SrcControl,
    start: StartTiming,
    transfer32: bool,
    enable: bool,
) -> u16 {
    let mut w = ((dest as u16) & 3) << 5;
    w |= ((src as u16) & 3) << 7;
    w |= ((start as u16) & 3) << 12;
    if transfer32 {
        w |= CONTROL_TRANSFER_TYPE;
    }
    if enable {
        w |= CONTROL_ENABLE;
    }
    w
}
