//! Colours for the painted UI, following the Windows "app mode" (light/dark).

use crate::ffi::HKEY_CURRENT_USER;
use crate::status::Light;
use crate::win::RegKey;

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub window: u32,
    pub card: u32,
    pub border: u32,
    pub text: u32,
    pub text2: u32,
    pub accent: u32,
    pub accent_text: u32,
    pub accent_pressed: u32,
    pub track: u32,
    pub used: u32,
}

pub const GREEN: u32 = 0xFF2E_A043;
pub const AMBER: u32 = 0xFFE8_A800;
pub const RED: u32 = 0xFFD1_3438;
pub const GREY: u32 = 0xFF8A_8A8A;

impl Default for Palette {
    fn default() -> Self {
        Palette::light()
    }
}

impl Palette {
    pub fn light() -> Palette {
        Palette {
            dark: false,
            window: 0xFFF3_F3F3,
            card: 0xFFFF_FFFF,
            border: 0xFFE0_E0E0,
            text: 0xFF1B_1B1B,
            text2: 0xFF61_6161,
            accent: 0xFF0F_6CBD,
            accent_text: 0xFFFF_FFFF,
            accent_pressed: 0xFF0C_3B5E,
            track: 0xFFE6_E6E6,
            used: 0xFF2B_88D8,
        }
    }

    pub fn dark() -> Palette {
        Palette {
            dark: true,
            window: 0xFF20_2020,
            card: 0xFF2B_2B2B,
            border: 0xFF3F_3F3F,
            text: 0xFFF2_F2F2,
            text2: 0xFFB8_B8B8,
            accent: 0xFF4C_C2FF,
            accent_text: 0xFF00_0000,
            accent_pressed: 0xFF2E_8FC4,
            track: 0xFF3D_3D3D,
            used: 0xFF4C_A8F0,
        }
    }

    /// Light unless Windows is set to dark mode for apps.
    pub fn detect() -> Palette {
        let dark = RegKey::open(
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        )
        .and_then(|k| k.dword("AppsUseLightTheme"))
            == Some(0);
        if dark {
            Palette::dark()
        } else {
            Palette::light()
        }
    }

    pub fn status(&self, l: Light) -> u32 {
        match l {
            Light::Green => GREEN,
            Light::Yellow => AMBER,
            Light::Red => RED,
            Light::Grey => GREY,
        }
    }

    /// Text colour for a status (amber is too light for text on white).
    pub fn status_text(&self, l: Light) -> u32 {
        match (l, self.dark) {
            (Light::Green, false) => 0xFF1A_7F37,
            (Light::Green, true) => 0xFF6C_CB5F,
            (Light::Yellow, false) => 0xFF9A_6700,
            (Light::Yellow, true) => 0xFFF2_C94C,
            (Light::Red, false) => 0xFFC4_2B1C,
            (Light::Red, true) => 0xFFFF_99A4,
            (Light::Grey, _) => self.text2,
        }
    }

    /// Soft background tint for a status badge.
    pub fn status_tint(&self, l: Light) -> u32 {
        mix(self.status(l), self.card, if self.dark { 0.28 } else { 0.16 })
    }
}

/// Blend `a` over `b` with weight `t` (0..1), result opaque.
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, s: u32| ((c >> s) & 0xFF) as f32;
    let m = |s: u32| ((ch(a, s) * t + ch(b, s) * (1.0 - t)).round() as u32) << s;
    0xFF00_0000 | m(16) | m(8) | m(0)
}

/// ARGB -> Win32 COLORREF (0x00BBGGRR).
pub fn colorref(argb: u32) -> u32 {
    ((argb & 0xFF) << 16) | (argb & 0xFF00) | ((argb >> 16) & 0xFF)
}
