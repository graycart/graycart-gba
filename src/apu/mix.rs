//! GBA mixer.
//!
//! Cited: GBATEK Sound Controller, SOUNDCNT_L, SOUNDCNT_H, SOUNDBIAS.
//! <https://problemkaputt.de/gbatek.htm>

/// Mixer inputs for one sample.
pub struct MixIn {
    pub psg: [i16; 4],
    pub fifo_a: i8,
    pub fifo_b: i8,
    pub cnt_l: u16,
    pub cnt_h: u16,
    pub bias: u16,
}

/// Mixer stereo output.
pub struct MixOut {
    pub left: i16,
    pub right: i16,
    pub clip: bool,
}

/// Mix one stereo sample from PSG and FIFO channels.
///
/// SOUNDCNT_H (GBATEK): bits 0–1 PSG output ratio; bit 2 FIFO A volume; bit 3 FIFO B
/// volume; bits 8–9 FIFO A right/left enable; bits 12–13 FIFO B right/left enable.
pub fn mix(input: &MixIn) -> MixOut {
    let right_vol = (input.cnt_l & 0x7) as i32;
    let left_vol = ((input.cnt_l >> 4) & 0x7) as i32;
    let right_en = (input.cnt_l >> 8) & 0xF;
    let left_en = (input.cnt_l >> 12) & 0xF;

    let psg_right = psg_ratio(psg_side(&input.psg, right_en, right_vol), input.cnt_h);
    let psg_left = psg_ratio(psg_side(&input.psg, left_en, left_vol), input.cnt_h);

    let fifo_a_scaled = fifo_volume(input.fifo_a, input.cnt_h & (1 << 2) != 0);
    let fifo_b_scaled = fifo_volume(input.fifo_b, input.cnt_h & (1 << 3) != 0);

    let fifo_a_right = if input.cnt_h & (1 << 8) != 0 {
        fifo_a_scaled
    } else {
        0
    };
    let fifo_a_left = if input.cnt_h & (1 << 9) != 0 {
        fifo_a_scaled
    } else {
        0
    };
    let fifo_b_right = if input.cnt_h & (1 << 12) != 0 {
        fifo_b_scaled
    } else {
        0
    };
    let fifo_b_left = if input.cnt_h & (1 << 13) != 0 {
        fifo_b_scaled
    } else {
        0
    };

    let bias_level = ((input.bias >> 1) & 0x1FF) as i32;
    let resolution = ((input.bias >> 14) & 3) as u32;

    let (right, clip_r) = finalize(
        psg_right + fifo_a_right + fifo_b_right,
        bias_level,
        resolution,
    );
    let (left, clip_l) = finalize(psg_left + fifo_a_left + fifo_b_left, bias_level, resolution);

    MixOut {
        left,
        right,
        clip: clip_l || clip_r,
    }
}

fn psg_side(psg: &[i16; 4], enables: u16, volume: i32) -> i32 {
    if enables == 0 {
        return 0;
    }
    let mut sum = 0i32;
    for (i, sample) in psg.iter().enumerate() {
        if enables & (1 << i) != 0 {
            sum += i32::from(*sample);
        }
    }
    sum * (volume + 1)
}

/// SOUNDCNT_H bits 0–1: 00 = 25%, 01 = 50%, 10/11 = 100%.
fn psg_ratio(side: i32, cnt_h: u16) -> i32 {
    match cnt_h & 0x3 {
        0 => side / 4,
        1 => side / 2,
        _ => side,
    }
}

fn fifo_volume(sample: i8, full: bool) -> i32 {
    let s = i32::from(sample);
    if full { s } else { s / 2 }
}

fn finalize(sum: i32, bias_level: i32, resolution: u32) -> (i16, bool) {
    let mut biased = sum + bias_level;
    biased &= !((1i32 << resolution) - 1);
    let centered = biased - bias_level;
    let clip = centered < i32::from(i16::MIN) || centered > i32::from(i16::MAX);
    let clamped = centered.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
    (clamped, clip)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input() -> MixIn {
        MixIn {
            psg: [0; 4],
            fifo_a: 0,
            fifo_b: 0,
            cnt_l: 0,
            cnt_h: 0,
            bias: 0,
        }
    }

    #[test]
    fn all_zeros_bias_zero() {
        let out = mix(&base_input());
        assert_eq!(out.left, 0);
        assert_eq!(out.right, 0);
        assert!(!out.clip);
    }

    #[test]
    fn psg_channel0_right_only_volume_zero() {
        let mut input = base_input();
        input.psg[0] = 15;
        // SOUNDCNT_L bit 8: channel 0 right enable; right volume bits 0-2 = 0
        // SOUNDCNT_H bits 0-1 = 10 => 100% PSG ratio
        input.cnt_l = 1 << 8;
        input.cnt_h = 0b10;
        let out = mix(&input);
        assert_eq!(out.right, 15);
        assert_eq!(out.left, 0);
        assert!(!out.clip);
    }

    #[test]
    fn psg_ratio_quarters_and_halves_side_sum() {
        let mut input = base_input();
        input.psg[0] = 16;
        input.cnt_l = 1 << 8; // right enable ch0, SOUNDCNT_L volume 0 => *1
        input.cnt_h = 0b00; // 25%
        assert_eq!(mix(&input).right, 4);
        input.cnt_h = 0b01; // 50%
        assert_eq!(mix(&input).right, 8);
        input.cnt_h = 0b10; // 100%
        assert_eq!(mix(&input).right, 16);
        input.cnt_h = 0b11; // prohibited → 100%
        assert_eq!(mix(&input).right, 16);
    }

    #[test]
    fn fifo_a_right_full_and_half_volume() {
        let mut input = base_input();
        input.fifo_a = 20;
        // bit 2 = A volume 100%; bit 8 = A right enable
        input.cnt_h = (1 << 2) | (1 << 8);
        let out = mix(&input);
        assert_eq!(out.right, 20);
        assert_eq!(out.left, 0);

        // bit 2 clear => 50%; bit 8 still enables right
        input.cnt_h = 1 << 8;
        let out = mix(&input);
        assert_eq!(out.right, 10);
        assert_eq!(out.left, 0);
    }

    #[test]
    fn sum_exceeding_i16_sets_clip() {
        let mut input = base_input();
        // One enabled PSG sample * (vol+1) far past i16::MAX
        input.psg[0] = 20_000;
        input.cnt_l = (1 << 8) | 7; // right enable ch0, volume 7 => *8
        input.cnt_h = 0b10; // 100% PSG ratio
        let out = mix(&input);
        assert!(out.clip);
        assert_eq!(out.right, i16::MAX);
    }

    #[test]
    fn bias_added_and_subtracted_centers_correctly() {
        let mut input = base_input();
        input.psg[0] = 1;
        input.cnt_l = 1 << 8; // right enable ch0, volume 0
        input.cnt_h = 0b10; // 100% PSG ratio
        input.bias = 0x0200; // level 0x100, resolution 0
        let out = mix(&input);
        assert_eq!(out.right, 1);
        assert_eq!(out.left, 0);
        assert!(!out.clip);
    }

    #[test]
    fn resolution_3_clears_low_bits() {
        let mut input = base_input();
        input.psg[0] = 1;
        input.cnt_l = 1 << 8;
        input.cnt_h = 0b10; // 100% PSG ratio
        input.bias = 0xC000; // resolution 3, bias level 0
        let out = mix(&input);
        assert_eq!(out.right, 0);
        assert_eq!(out.left, 0);
        assert!(!out.clip);
    }
}
