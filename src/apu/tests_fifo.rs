//! G6-fifo — FIFO A/B timer clock + DMA request + reset.
//!
//! Cited: GBATEK — FIFO / DMA Sound
//!   https://problemkaputt.de/gbatek.htm

use super::fifo::{FIFO_CAPACITY, FIFO_HALF};
use super::regs::{MASTER_ENABLE, OFF_FIFO_A, OFF_FIFO_B, OFF_SOUNDCNT_H, OFF_SOUNDCNT_X};
use super::Apu;

#[test]
fn fifo_reset_bit_clears_queue() {
    let mut apu = Apu::new();
    apu.write32(OFF_FIFO_A, 0x0102_0304);
    assert!(!apu.fifos.a.is_empty());
    apu.write16(OFF_SOUNDCNT_H, 1 << 11); // reset A
    assert!(apu.fifos.a.is_empty());
    assert_eq!(apu.fifos.latch_a, 0);
}

#[test]
fn timer0_overflow_pops_and_requests_dma_when_half() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    // Timer0 for A (bit10=0), route irrelevant
    apu.write16(OFF_SOUNDCNT_H, 0x0200); // A→right only, TM0
                                         // Fill just above half so one pop crosses threshold
    for i in 0..(FIFO_HALF + 1) {
        apu.fifos.a.push_sample(i as i8);
    }
    assert!(!apu.fifos.a.needs_dma());
    let _ = apu.take_fifo_dma_request();
    apu.on_timer_overflows(1, 0);
    assert_eq!(apu.fifos.latch_a, 0); // oldest sample
    assert_eq!(apu.fifos.a.len(), FIFO_HALF);
    let req = apu.take_fifo_dma_request();
    assert_eq!(req & 1, 1, "DMA1 should request when ≤ half");
}

#[test]
fn timer1_selected_for_fifo_b() {
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_H, (1 << 14) | (1 << 12)); // B timer1 + right
    for _ in 0..4 {
        apu.fifos.b.push_sample(0x55);
    }
    let _ = apu.take_fifo_dma_request();
    apu.on_timer_overflows(5, 0); // TM0 only — should not pop B
    assert_eq!(apu.fifos.b.len(), 4);
    apu.on_timer_overflows(0, 1);
    assert_eq!(apu.fifos.b.len(), 3);
    assert_eq!(apu.fifos.latch_b, 0x55);
}

#[test]
fn underrun_holds_last_sample() {
    let mut apu = Apu::new();
    apu.fifos.a.push_sample(0x7F);
    apu.on_timer_overflows(1, 0);
    assert_eq!(apu.fifos.latch_a, 0x7F);
    // Empty further pops hold last
    for _ in 0..8 {
        apu.on_timer_overflows(1, 0);
    }
    assert_eq!(apu.fifos.latch_a, 0x7F);
}

#[test]
fn fifo_capacity_is_32_samples() {
    let mut apu = Apu::new();
    for i in 0..32 {
        apu.fifos.a.push_sample(i as i8);
    }
    assert_eq!(apu.fifos.a.len(), FIFO_CAPACITY);
}

#[test]
fn fifo_overflow_resets_empty_like_hw() {
    // Gericom / mGBA #1847: overflow clears the FIFO (like the reset bit),
    // then accepts the new write. Drop-oldest corrupts the playhead into a
    // harsh saw while DMA keeps feeding.
    let mut apu = Apu::new();
    for i in 0..FIFO_CAPACITY {
        apu.fifos.a.push_sample(i as i8);
    }
    assert_eq!(apu.fifos.a.len(), FIFO_CAPACITY);
    apu.fifos.a.push_sample(99);
    assert_eq!(
        apu.fifos.a.len(),
        1,
        "overflow must reset empty then keep the new sample"
    );
    assert_eq!(apu.fifos.a.pop_sample(), 99);
}

#[test]
fn fifo_b_word_push() {
    let mut apu = Apu::new();
    apu.write32(OFF_FIFO_B, 0xAABB_CCDD);
    assert_eq!(apu.fifos.b.pop_sample() as u8, 0xDD);
    assert_eq!(apu.fifos.b.pop_sample() as u8, 0xCC);
}

#[test]
fn synthetic_fifo_stream_signed_pcm_has_energy() {
    // Drive FIFO A with a repeating signed ramp via word pushes, clock with TM0,
    // mix to PCM — must be non-silent and not stuck at bias center.
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_X, MASTER_ENABLE);
    // A → L+R, full volume, TM0; PSG ratio irrelevant
    apu.write16(OFF_SOUNDCNT_H, 0x0B04);
    // Signed ramp bytes: -128,-64,0,64 repeated
    let word = ((-128i8) as u8 as u32)
        | (((-64i8) as u8 as u32) << 8)
        | ((0u8 as u32) << 16)
        | ((64u8 as u32) << 24);
    for _ in 0..8 {
        apu.fifos.a.push_word(word);
    }
    // Pop through several timer edges and emit PWM frames
    for _ in 0..64 {
        apu.on_timer_overflows(1, 0);
        apu.step(512); // one PWM period at default bias
    }
    let frames = apu.pcm.snapshot();
    assert!(frames.len() >= 32);
    let rms = super::pcm::soft_rms(&frames);
    assert!(
        rms > 100.0,
        "synthetic FIFO PCM should have energy, rms={rms}"
    );
    // Latched sample must stay signed (not reinterpreted as unsigned mid)
    assert!(
        apu.fifos.latch_a == 64
            || apu.fifos.latch_a == -128
            || apu.fifos.latch_a == -64
            || apu.fifos.latch_a == 0
    );
}

#[test]
fn batched_timer_overflows_request_dma_each_half_crossing() {
    // Many overflows in one call must keep requesting DMA whenever ≤ half —
    // glue must refill between pops (tested via request bit after batch).
    let mut apu = Apu::new();
    apu.write16(OFF_SOUNDCNT_H, 0); // A uses TM0
                                    // Exactly half+1 so first pop crosses threshold; further pops stay ≤ half.
    for i in 0..(FIFO_HALF + 1) {
        apu.fifos.a.push_sample(i as i8);
    }
    let _ = apu.take_fifo_dma_request();
    apu.on_timer_overflows(20, 0);
    assert!(apu.fifos.a.len() < FIFO_HALF);
    let req = apu.take_fifo_dma_request();
    assert_eq!(req & 1, 1);
}
