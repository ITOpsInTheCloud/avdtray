//! GDI+ flat API (gdiplus.dll ships with every Windows version) plus a tiny
//! safe drawing wrapper used by the painted status view.
#![allow(non_snake_case, dead_code)]

use crate::ffi::{BOOL, HDC};
use crate::win::wide;
use core::ffi::c_void;
use std::ptr::{null, null_mut};

type Gp = *mut c_void;
type Status = i32;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectF {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        RectF { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn inset(&self, dx: f32, dy: f32) -> RectF {
        RectF::new(self.x + dx, self.y + dy, (self.w - 2.0 * dx).max(0.0), (self.h - 2.0 * dy).max(0.0))
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
}

#[repr(C)]
struct StartupInput {
    version: u32,
    debug_cb: *const c_void,
    suppress_bg_thread: BOOL,
    suppress_codecs: BOOL,
}

#[link(name = "gdiplus")]
extern "system" {
    fn GdiplusStartup(token: *mut usize, input: *const StartupInput, output: *mut c_void) -> Status;
    fn GdiplusShutdown(token: usize);
    fn GdipCreateFromHDC(hdc: HDC, g: *mut Gp) -> Status;
    fn GdipDeleteGraphics(g: Gp) -> Status;
    fn GdipSetSmoothingMode(g: Gp, mode: i32) -> Status;
    fn GdipSetTextRenderingHint(g: Gp, hint: i32) -> Status;
    fn GdipSetPixelOffsetMode(g: Gp, mode: i32) -> Status;
    fn GdipGraphicsClear(g: Gp, argb: u32) -> Status;
    fn GdipSetClipRect(g: Gp, x: f32, y: f32, w: f32, h: f32, mode: i32) -> Status;
    fn GdipResetClip(g: Gp) -> Status;

    fn GdipCreateSolidFill(argb: u32, brush: *mut Gp) -> Status;
    fn GdipDeleteBrush(brush: Gp) -> Status;
    fn GdipCreatePen1(argb: u32, width: f32, unit: i32, pen: *mut Gp) -> Status;
    fn GdipDeletePen(pen: Gp) -> Status;
    fn GdipSetPenStartCap(pen: Gp, cap: i32) -> Status;
    fn GdipSetPenEndCap(pen: Gp, cap: i32) -> Status;

    fn GdipFillRectangle(g: Gp, brush: Gp, x: f32, y: f32, w: f32, h: f32) -> Status;
    fn GdipFillEllipse(g: Gp, brush: Gp, x: f32, y: f32, w: f32, h: f32) -> Status;
    fn GdipDrawEllipse(g: Gp, pen: Gp, x: f32, y: f32, w: f32, h: f32) -> Status;
    fn GdipDrawLine(g: Gp, pen: Gp, x1: f32, y1: f32, x2: f32, y2: f32) -> Status;
    fn GdipDrawArc(g: Gp, pen: Gp, x: f32, y: f32, w: f32, h: f32, start: f32, sweep: f32) -> Status;
    fn GdipFillPie(g: Gp, brush: Gp, x: f32, y: f32, w: f32, h: f32, start: f32, sweep: f32) -> Status;

    fn GdipCreatePath(fill_mode: i32, path: *mut Gp) -> Status;
    fn GdipDeletePath(path: Gp) -> Status;
    fn GdipAddPathArc(path: Gp, x: f32, y: f32, w: f32, h: f32, start: f32, sweep: f32) -> Status;
    fn GdipClosePathFigure(path: Gp) -> Status;
    fn GdipFillPath(g: Gp, brush: Gp, path: Gp) -> Status;
    fn GdipDrawPath(g: Gp, pen: Gp, path: Gp) -> Status;

    fn GdipCreateFontFamilyFromName(name: *const u16, collection: *mut c_void, family: *mut Gp) -> Status;
    fn GdipDeleteFontFamily(family: Gp) -> Status;
    fn GdipCreateFont(family: Gp, size: f32, style: i32, unit: i32, font: *mut Gp) -> Status;
    fn GdipDeleteFont(font: Gp) -> Status;

    fn GdipCreateStringFormat(attrs: i32, lang: u16, fmt: *mut Gp) -> Status;
    fn GdipDeleteStringFormat(fmt: Gp) -> Status;
    fn GdipSetStringFormatAlign(fmt: Gp, align: i32) -> Status;
    fn GdipSetStringFormatLineAlign(fmt: Gp, align: i32) -> Status;
    fn GdipSetStringFormatTrimming(fmt: Gp, trimming: i32) -> Status;
    fn GdipSetStringFormatFlags(fmt: Gp, flags: i32) -> Status;
    fn GdipDrawString(g: Gp, text: *const u16, len: i32, font: Gp, layout: *const RectF, fmt: Gp, brush: Gp) -> Status;
    fn GdipMeasureString(
        g: Gp,
        text: *const u16,
        len: i32,
        font: Gp,
        layout: *const RectF,
        fmt: Gp,
        bbox: *mut RectF,
        chars: *mut i32,
        lines: *mut i32,
    ) -> Status;
}

const UNIT_PIXEL: i32 = 2;
const SMOOTHING_ANTIALIAS: i32 = 4;
const PIXEL_OFFSET_HALF: i32 = 4;
const TEXT_CLEARTYPE_GRIDFIT: i32 = 5;
const LINECAP_ROUND: i32 = 2;
const FLAG_NOWRAP: i32 = 0x1000;
const TRIM_ELLIPSIS_CHAR: i32 = 3;
const TRIM_ELLIPSIS_PATH: i32 = 5;

/// Keeps GDI+ alive for the lifetime of the process.
pub struct Gdiplus(usize);

impl Gdiplus {
    pub fn start() -> Option<Gdiplus> {
        let input = StartupInput { version: 1, debug_cb: null(), suppress_bg_thread: 0, suppress_codecs: 0 };
        let mut token = 0usize;
        let st = unsafe { GdiplusStartup(&mut token, &input, null_mut()) };
        if st == 0 {
            Some(Gdiplus(token))
        } else {
            None
        }
    }
}

impl Drop for Gdiplus {
    fn drop(&mut self) {
        unsafe { GdiplusShutdown(self.0) }
    }
}

// ------------------------------------------------------------------ fonts

pub struct Font(Gp);

impl Font {
    /// `px` is the em size in device pixels. Falls back to Segoe UI / Arial.
    pub fn new(families: &[&str], px: f32, bold: bool) -> Font {
        unsafe {
            for name in families.iter().chain(["Segoe UI", "Arial"].iter()) {
                let n = wide(name);
                let mut fam: Gp = null_mut();
                if GdipCreateFontFamilyFromName(n.as_ptr(), null_mut(), &mut fam) == 0 && !fam.is_null() {
                    let mut f: Gp = null_mut();
                    let st = GdipCreateFont(fam, px, if bold { 1 } else { 0 }, UNIT_PIXEL, &mut f);
                    GdipDeleteFontFamily(fam);
                    if st == 0 && !f.is_null() {
                        return Font(f);
                    }
                }
            }
        }
        Font(null_mut())
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { GdipDeleteFont(self.0) };
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Near,
    Center,
    Far,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trim {
    End,
    Path,
}

// ------------------------------------------------------------------ graphics

pub struct Graphics(Gp);

impl Graphics {
    pub fn from_hdc(hdc: HDC) -> Option<Graphics> {
        let mut g: Gp = null_mut();
        unsafe {
            if GdipCreateFromHDC(hdc, &mut g) != 0 || g.is_null() {
                return None;
            }
            GdipSetSmoothingMode(g, SMOOTHING_ANTIALIAS);
            GdipSetPixelOffsetMode(g, PIXEL_OFFSET_HALF);
            GdipSetTextRenderingHint(g, TEXT_CLEARTYPE_GRIDFIT);
        }
        Some(Graphics(g))
    }

    pub fn clear(&self, argb: u32) {
        unsafe { GdipGraphicsClear(self.0, argb) };
    }

    pub fn clip(&self, r: RectF) {
        unsafe { GdipSetClipRect(self.0, r.x, r.y, r.w, r.h, 0) };
    }

    pub fn reset_clip(&self) {
        unsafe { GdipResetClip(self.0) };
    }

    fn with_brush(&self, argb: u32, f: impl FnOnce(Gp)) {
        let mut b: Gp = null_mut();
        unsafe {
            if GdipCreateSolidFill(argb, &mut b) == 0 && !b.is_null() {
                f(b);
                GdipDeleteBrush(b);
            }
        }
    }

    fn with_pen(&self, argb: u32, width: f32, round: bool, f: impl FnOnce(Gp)) {
        let mut p: Gp = null_mut();
        unsafe {
            if GdipCreatePen1(argb, width, UNIT_PIXEL, &mut p) == 0 && !p.is_null() {
                if round {
                    GdipSetPenStartCap(p, LINECAP_ROUND);
                    GdipSetPenEndCap(p, LINECAP_ROUND);
                }
                f(p);
                GdipDeletePen(p);
            }
        }
    }

    pub fn fill_rect(&self, r: RectF, argb: u32) {
        self.with_brush(argb, |b| unsafe {
            GdipFillRectangle(self.0, b, r.x, r.y, r.w, r.h);
        });
    }

    fn round_path(r: RectF, radius: f32) -> Gp {
        let mut path: Gp = null_mut();
        unsafe {
            if GdipCreatePath(0, &mut path) != 0 || path.is_null() {
                return null_mut();
            }
            let d = (radius * 2.0).min(r.w).min(r.h).max(0.1);
            GdipAddPathArc(path, r.x, r.y, d, d, 180.0, 90.0);
            GdipAddPathArc(path, r.right() - d, r.y, d, d, 270.0, 90.0);
            GdipAddPathArc(path, r.right() - d, r.bottom() - d, d, d, 0.0, 90.0);
            GdipAddPathArc(path, r.x, r.bottom() - d, d, d, 90.0, 90.0);
            GdipClosePathFigure(path);
        }
        path
    }

    /// Rounded rectangle; `fill` and/or `border` (0 = skip).
    pub fn round_rect(&self, r: RectF, radius: f32, fill: u32, border: u32, border_w: f32) {
        let path = Self::round_path(r, radius);
        if path.is_null() {
            return;
        }
        unsafe {
            if fill != 0 {
                self.with_brush(fill, |b| {
                    GdipFillPath(self.0, b, path);
                });
            }
            if border != 0 {
                self.with_pen(border, border_w, false, |p| {
                    GdipDrawPath(self.0, p, path);
                });
            }
            GdipDeletePath(path);
        }
    }

    pub fn fill_circle(&self, cx: f32, cy: f32, r: f32, argb: u32) {
        self.with_brush(argb, |b| unsafe {
            GdipFillEllipse(self.0, b, cx - r, cy - r, 2.0 * r, 2.0 * r);
        });
    }

    pub fn circle(&self, cx: f32, cy: f32, r: f32, argb: u32, width: f32) {
        self.with_pen(argb, width, false, |p| unsafe {
            GdipDrawEllipse(self.0, p, cx - r, cy - r, 2.0 * r, 2.0 * r);
        });
    }

    pub fn fill_ellipse(&self, r: RectF, argb: u32) {
        self.with_brush(argb, |b| unsafe {
            GdipFillEllipse(self.0, b, r.x, r.y, r.w, r.h);
        });
    }

    pub fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, argb: u32, width: f32) {
        self.with_pen(argb, width, true, |p| unsafe {
            GdipDrawLine(self.0, p, x1, y1, x2, y2);
        });
    }

    /// Arc around a circle centre; angles in degrees, 0 = 3 o'clock, clockwise.
    pub fn arc(&self, cx: f32, cy: f32, r: f32, start: f32, sweep: f32, argb: u32, width: f32) {
        if sweep.abs() < 0.01 {
            return;
        }
        self.with_pen(argb, width, false, |p| unsafe {
            GdipDrawArc(self.0, p, cx - r, cy - r, 2.0 * r, 2.0 * r, start, sweep);
        });
    }

    pub fn pie(&self, r: RectF, start: f32, sweep: f32, argb: u32) {
        self.with_brush(argb, |b| unsafe {
            GdipFillPie(self.0, b, r.x, r.y, r.w, r.h, start, sweep);
        });
    }

    fn with_format(&self, h: Align, v: Align, trim: Trim, f: impl FnOnce(Gp)) {
        let mut fmt: Gp = null_mut();
        unsafe {
            if GdipCreateStringFormat(0, 0, &mut fmt) != 0 || fmt.is_null() {
                return;
            }
            let a = |x: Align| match x {
                Align::Near => 0,
                Align::Center => 1,
                Align::Far => 2,
            };
            GdipSetStringFormatAlign(fmt, a(h));
            GdipSetStringFormatLineAlign(fmt, a(v));
            GdipSetStringFormatFlags(fmt, FLAG_NOWRAP);
            GdipSetStringFormatTrimming(fmt, if trim == Trim::Path { TRIM_ELLIPSIS_PATH } else { TRIM_ELLIPSIS_CHAR });
            f(fmt);
            GdipDeleteStringFormat(fmt);
        }
    }

    /// Single-line text, vertically centred in `r`, ellipsised if too long.
    pub fn text(&self, s: &str, font: &Font, r: RectF, argb: u32, h: Align, trim: Trim) {
        if s.is_empty() || font.0.is_null() || r.w <= 0.0 {
            return;
        }
        let w: Vec<u16> = s.encode_utf16().collect();
        self.with_format(h, Align::Center, trim, |fmt| {
            self.with_brush(argb, |b| unsafe {
                GdipDrawString(self.0, w.as_ptr(), w.len() as i32, font.0, &r, fmt, b);
            });
        });
    }

    /// Width of a single line of text in pixels.
    pub fn measure(&self, s: &str, font: &Font) -> f32 {
        if s.is_empty() || font.0.is_null() {
            return 0.0;
        }
        let w: Vec<u16> = s.encode_utf16().collect();
        let layout = RectF::new(0.0, 0.0, 10_000.0, 1_000.0);
        let mut bbox = RectF::default();
        let mut out = 0.0;
        self.with_format(Align::Near, Align::Near, Trim::End, |fmt| unsafe {
            if GdipMeasureString(self.0, w.as_ptr(), w.len() as i32, font.0, &layout, fmt, &mut bbox, null_mut(), null_mut()) == 0 {
                out = bbox.w;
            }
        });
        out
    }
}

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe { GdipDeleteGraphics(self.0) };
    }
}
