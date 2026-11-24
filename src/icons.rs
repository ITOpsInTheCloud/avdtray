//! Traffic-light tray icons drawn at runtime (no .ico resources needed, crisp at any DPI).
//! Each state also carries a white glyph so it is distinguishable without color:
//!   green = plain, yellow = "!", red = bar ("no entry"), grey = hollow ring.

use crate::ffi::*;
use crate::status::Light;
use core::ffi::c_void;
use std::ptr::null_mut;

pub fn make(light: Light, size: i32) -> HICON {
    let size = size.clamp(16, 256);
    let (r, g, b) = match light {
        Light::Green => (0x2E, 0xA0, 0x43),
        Light::Yellow => (0xE8, 0xA8, 0x00),
        Light::Red => (0xD1, 0x34, 0x38),
        Light::Grey => (0x8A, 0x8A, 0x8A),
    };

    unsafe {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [0],
        };
        let mut bits: *mut c_void = null_mut();
        let color = CreateDIBSection(0, &bmi, DIB_RGB_COLORS, &mut bits, 0, 0);
        if color == 0 || bits.is_null() {
            return 0;
        }
        let px = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);

        let s = size as f32;
        let c = s / 2.0;
        let radius = c - s * 0.04;
        let ring = (s / 14.0).max(1.0);
        let glyph = (s * 0.075).max(1.0); // half-width of glyph strokes

        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 + 0.5 - c;
                let dy = y as f32 + 0.5 - c;
                let d = (dx * dx + dy * dy).sqrt();
                let cover = (radius + 0.5 - d).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    px[(y * size + x) as usize] = 0;
                    continue;
                }
                let (mut cr, mut cg, mut cb) = (r as f32, g as f32, b as f32);
                let mut alpha = cover;

                if d > radius - ring {
                    // darker rim so the dot reads on light and dark taskbars
                    cr *= 0.62;
                    cg *= 0.62;
                    cb *= 0.62;
                } else {
                    // soft top-left highlight
                    let f = 1.0 + 0.22 * (-(dx + dy) / (2.0 * radius));
                    cr = (cr * f).min(255.0);
                    cg = (cg * f).min(255.0);
                    cb = (cb * f).min(255.0);

                    let white = match light {
                        Light::Red => dy.abs() < glyph && dx.abs() < radius * 0.55,
                        Light::Yellow => {
                            (dx.abs() < glyph && dy > -radius * 0.55 && dy < radius * 0.12)
                                || (dx.abs() < glyph && dy > radius * 0.28 && dy < radius * 0.28 + glyph * 2.0)
                        }
                        Light::Grey => d < radius - ring * 2.5,
                        Light::Green => false,
                    };
                    if white {
                        if light == Light::Grey {
                            alpha = 0.0; // hollow center
                        } else {
                            cr = 255.0;
                            cg = 255.0;
                            cb = 255.0;
                        }
                    }
                }
                let a = (alpha * 255.0) as u32;
                px[(y * size + x) as usize] = (a << 24) | ((cr as u32) << 16) | ((cg as u32) << 8) | (cb as u32);
            }
        }

        // All-zero monochrome mask: the alpha channel of the color bitmap decides transparency.
        let row_bytes = (((size + 15) / 16) * 2) as usize;
        let mask_bits = vec![0u8; row_bytes * size as usize];
        let mask = CreateBitmap(size, size, 1, 1, mask_bits.as_ptr() as *const c_void);

        let info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info);
        DeleteObject(color);
        if mask != 0 {
            DeleteObject(mask);
        }
        icon
    }
}
