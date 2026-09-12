//! Unit tests for video STRB / write-size policy (`video.rs`).

use super::region::VRAM_SIZE;
use super::video::{
    classify_vram_offset, expand_strb_byte, obj_vram_base, resolve_strb, resolve_video_write,
    resolve_wide_write, write_size_ok, VideoTarget, VideoWriteAction, OBJ_VRAM_BASE_BITMAP,
    OBJ_VRAM_BASE_TILE,
};
use super::AccessSize;

#[test]
fn expand_strb_duplicates_byte() {
    assert_eq!(expand_strb_byte(0x00), 0x0000);
    assert_eq!(expand_strb_byte(0xAB), 0xABAB);
    assert_eq!(expand_strb_byte(0xFF), 0xFFFF);
    // data × 0x0101
    assert_eq!(expand_strb_byte(0x12), 0x12u16.wrapping_mul(0x0101));
}

#[test]
fn obj_vram_base_by_mode() {
    assert_eq!(obj_vram_base(0), OBJ_VRAM_BASE_TILE);
    assert_eq!(obj_vram_base(1), OBJ_VRAM_BASE_TILE);
    assert_eq!(obj_vram_base(2), OBJ_VRAM_BASE_TILE);
    assert_eq!(obj_vram_base(3), OBJ_VRAM_BASE_BITMAP);
    assert_eq!(obj_vram_base(4), OBJ_VRAM_BASE_BITMAP);
    assert_eq!(obj_vram_base(5), OBJ_VRAM_BASE_BITMAP);
}

#[test]
fn classify_vram_tile_modes() {
    assert_eq!(classify_vram_offset(0x0000, 0), Some(VideoTarget::BgVram));
    assert_eq!(classify_vram_offset(0xFFFF, 2), Some(VideoTarget::BgVram));
    assert_eq!(
        classify_vram_offset(0x1_0000, 0),
        Some(VideoTarget::ObjVram)
    );
    assert_eq!(
        classify_vram_offset(0x1_7FFF, 1),
        Some(VideoTarget::ObjVram)
    );
    assert_eq!(classify_vram_offset(VRAM_SIZE as u32, 0), None);
}

#[test]
fn classify_vram_bitmap_modes() {
    assert_eq!(classify_vram_offset(0x1_3FFF, 3), Some(VideoTarget::BgVram));
    assert_eq!(
        classify_vram_offset(0x1_4000, 3),
        Some(VideoTarget::ObjVram)
    );
    // In tile modes the same offset sits in OBJ VRAM (base 0x10000).
    assert_eq!(
        classify_vram_offset(0x1_3FFF, 0),
        Some(VideoTarget::ObjVram)
    );
    // Bitmap modes still treat 0x10000..0x13FFF as BG.
    assert_eq!(classify_vram_offset(0x1_0000, 3), Some(VideoTarget::BgVram));
}

#[test]
fn write_size_16_32_only() {
    for target in [
        VideoTarget::Palette,
        VideoTarget::Oam,
        VideoTarget::BgVram,
        VideoTarget::ObjVram,
    ] {
        assert!(!write_size_ok(target, AccessSize::Byte));
        assert!(write_size_ok(target, AccessSize::Half));
        assert!(write_size_ok(target, AccessSize::Word));
        assert_eq!(
            resolve_wide_write(target, AccessSize::Half),
            VideoWriteAction::Store
        );
        assert_eq!(
            resolve_wide_write(target, AccessSize::Byte),
            VideoWriteAction::Reject
        );
    }
}

#[test]
fn strb_obj_vram_and_oam_ignored() {
    assert_eq!(
        resolve_strb(VideoTarget::ObjVram, 0xAA),
        VideoWriteAction::Ignore
    );
    assert_eq!(
        resolve_strb(VideoTarget::Oam, 0x55),
        VideoWriteAction::Ignore
    );
}

#[test]
fn strb_bg_vram_and_palette_expand() {
    assert_eq!(
        resolve_strb(VideoTarget::BgVram, 0xCD),
        VideoWriteAction::ExpandByteToHalfword { halfword: 0xCDCD }
    );
    assert_eq!(
        resolve_strb(VideoTarget::Palette, 0x11),
        VideoWriteAction::ExpandByteToHalfword { halfword: 0x1111 }
    );
}

#[test]
fn resolve_video_write_routes_sizes() {
    assert_eq!(
        resolve_video_write(VideoTarget::Oam, AccessSize::Byte, 0x77),
        VideoWriteAction::Ignore
    );
    assert_eq!(
        resolve_video_write(VideoTarget::Palette, AccessSize::Byte, 0x22),
        VideoWriteAction::ExpandByteToHalfword { halfword: 0x2222 }
    );
    assert_eq!(
        resolve_video_write(VideoTarget::BgVram, AccessSize::Word, 0),
        VideoWriteAction::Store
    );
}
