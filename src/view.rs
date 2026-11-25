//! The painted status window: header cards, tab strip, container panels with a
//! free-space donut, and owner-drawn buttons. Everything is drawn with GDI+ into
//! an off-screen bitmap (see main.rs), so it is flicker-free and DPI-crisp.

use crate::gdip::{Align, Font, Graphics, RectF, Trim};
use crate::status::{self, Container, Light, Snapshot};
use crate::theme::{mix, Palette};
use crate::win;

pub const TAB_PROFILE: usize = 0;
pub const TAB_ODFC: usize = 1;
pub const TAB_DETAILS: usize = 2;
pub const TAB_REPORT: usize = 3;
pub const TAB_COUNT: usize = 4;
const TAB_LABELS: [&str; TAB_COUNT] = ["Profile Container", "Office Container (ODFC)", "Session Details", "Report"];

/// Layout shared by painting, hit-testing and child-control placement.
pub struct Geo {
    pub s: f32,
    pub title: RectF,
    pub cards: RectF,
    pub tabs: RectF,
    pub content: RectF,
    /// Area for the ListView / report edit control inside the content card.
    pub inner: RectF,
    pub buttons_y: f32,
    pub button_h: f32,
    pub button_w: f32,
    pub pad: f32,
}

pub fn geometry(w: f32, h: f32, dpi: u32) -> Geo {
    let s = dpi as f32 / 96.0;
    let pad = 16.0 * s;
    let iw = (w - 2.0 * pad).max(10.0);
    let title = RectF::new(pad, pad * 0.75, iw, 46.0 * s);
    let cards = RectF::new(pad, title.bottom() + 6.0 * s, iw, 72.0 * s);
    let tabs = RectF::new(pad, cards.bottom() + 14.0 * s, iw, 40.0 * s);
    let button_h = 32.0 * s;
    let buttons_y = h - pad - button_h;
    let content = RectF::new(pad, tabs.bottom(), iw, (buttons_y - 12.0 * s - tabs.bottom()).max(40.0 * s));
    let inner = content.inset(12.0 * s, 12.0 * s);
    Geo { s, title, cards, tabs, content, inner, buttons_y, button_h, button_w: 118.0 * s, pad }
}

pub struct ViewData<'a> {
    pub snap: Option<&'a Snapshot>,
    pub generated: &'a str,
    pub tab: usize,
    pub pal: Palette,
}

struct Fonts {
    title: Font,
    small: Font,
    value: Font,
    tab: Font,
    tab_sel: Font,
    section: Font,
    grid: Font,
    badge: Font,
    big: Font,
}

impl Fonts {
    fn new(s: f32) -> Fonts {
        let semi = ["Segoe UI Semibold"];
        let reg = ["Segoe UI"];
        Fonts {
            title: Font::new(&semi, 19.0 * s, false),
            small: Font::new(&reg, 12.0 * s, false),
            value: Font::new(&semi, 15.0 * s, false),
            tab: Font::new(&reg, 13.5 * s, false),
            tab_sel: Font::new(&semi, 13.5 * s, false),
            section: Font::new(&semi, 16.0 * s, false),
            grid: Font::new(&reg, 13.0 * s, false),
            badge: Font::new(&semi, 12.5 * s, false),
            big: Font::new(&["Segoe UI"], 28.0 * s, true),
        }
    }
}

/// What the window needs after painting: tab hit areas and tooltip areas.
#[derive(Default)]
pub struct PaintOut {
    pub tabs: Vec<RectF>,
    /// Areas whose text was cut off, with the full text for a hover tooltip.
    pub hovers: Vec<(RectF, String)>,
}

/// Remember `r` for a tooltip if `text` does not fit into it.
fn hover_if_cut(g: &Graphics, hovers: &mut Vec<(RectF, String)>, text: &str, font: &Font, r: RectF) {
    if !text.is_empty() && g.measure(text, font) > r.w + 0.5 {
        hovers.push((r, text.to_string()));
    }
}

/// Paints the whole client area.
pub fn paint(g: &Graphics, w: f32, h: f32, dpi: u32, d: &ViewData) -> PaintOut {
    let mut hovers: Vec<(RectF, String)> = Vec::new();
    let geo = geometry(w, h, dpi);
    let s = geo.s;
    let p = d.pal;
    let f = Fonts::new(s);
    g.clear(p.window);

    // ---- title
    let (t1, t2) = geo.title.split_rows(0.6);
    g.text(
        &format!("AVD Tray {} \u{2013} FSLogix status report", env!("CARGO_PKG_VERSION")),
        &f.title,
        t1,
        p.text,
        Align::Near,
        Trim::End,
    );
    g.text(&format!("Generated:  {}", d.generated), &f.small, t2, p.text2, Align::Near, Trim::End);

    // ---- header cards
    g.round_rect(geo.cards, 8.0 * s, p.card, p.border, 1.0);
    // computer, user, session id, overall status (overall gets the most room)
    const WEIGHTS: [f32; 4] = [0.25, 0.22, 0.17, 0.36];
    let (computer, user, session, light, overall) = match d.snap {
        Some(sn) => (
            sn.computer.clone(),
            short_user(&sn.user),
            sn.session.map_or("-".into(), |v| v.to_string()),
            sn.light,
            overall_value(sn),
        ),
        None => ("-".into(), "-".into(), "-".into(), Light::Grey, "Loading\u{2026}".into()),
    };
    let mut cx0 = geo.cards.x;
    for (i, wt) in WEIGHTS.iter().enumerate() {
        let cell = RectF::new(cx0, geo.cards.y, geo.cards.w * wt, geo.cards.h);
        cx0 = cell.right();
        if i > 0 {
            g.line(cell.x, cell.y + 14.0 * s, cell.x, cell.bottom() - 14.0 * s, p.border, 1.0);
        }
        let icx = cell.x + 18.0 * s + 14.0 * s;
        let icy = cell.y + cell.h / 2.0;
        let isz = 28.0 * s;
        let (label, value, color) = match i {
            0 => {
                icon_monitor(g, icx, icy, isz, p.accent, p.card);
                ("Computer", computer.as_str(), p.text)
            }
            1 => {
                icon_user(g, icx, icy, isz, p.accent);
                ("User", user.as_str(), p.text)
            }
            2 => {
                icon_users(g, icx, icy, isz, p.accent, p.card);
                ("Session ID", session.as_str(), p.text)
            }
            _ => {
                status_glyph(g, light, icx, icy, 13.0 * s);
                ("Overall status", overall.as_str(), p.status_text(light))
            }
        };
        let tx = icx + 14.0 * s + 12.0 * s;
        let tw = (cell.right() - tx - 10.0 * s).max(0.0);
        g.text(label, &f.small, RectF::new(tx, cell.y + 12.0 * s, tw, 20.0 * s), p.text2, Align::Near, Trim::End);
        let vr = RectF::new(tx, cell.y + 34.0 * s, tw, 24.0 * s);
        g.text(value, &f.value, vr, color, Align::Near, Trim::End);
        hover_if_cut(g, &mut hovers, value, &f.value, vr);
    }

    // ---- tabs
    let mut rects = Vec::with_capacity(TAB_COUNT);
    let mut x = geo.tabs.x;
    for (i, label) in TAB_LABELS.iter().enumerate() {
        let sel = i == d.tab;
        let font = if sel { &f.tab_sel } else { &f.tab };
        let tw = g.measure(label, font).max(40.0 * s);
        let r = RectF::new(x, geo.tabs.y, 16.0 * s + 18.0 * s + 8.0 * s + tw + 16.0 * s, geo.tabs.h);
        if sel {
            g.round_rect(RectF::new(r.x, r.y, r.w, r.h + 8.0 * s), 8.0 * s, p.card, p.border, 1.0);
        }
        let col = if sel { p.accent } else { p.text2 };
        let icx = r.x + 16.0 * s + 9.0 * s;
        let icy = r.y + r.h / 2.0;
        let isz = 18.0 * s;
        match i {
            TAB_PROFILE => icon_user(g, icx, icy, isz, col),
            TAB_ODFC => icon_briefcase(g, icx, icy, isz, col),
            TAB_DETAILS => icon_info(g, icx, icy, isz, col),
            _ => icon_lines(g, icx, icy, isz, col),
        }
        g.text(
            label,
            font,
            RectF::new(icx + 9.0 * s + 8.0 * s, r.y, tw + 4.0 * s, r.h),
            if sel { p.text } else { p.text2 },
            Align::Near,
            Trim::End,
        );
        rects.push(r);
        x = r.right() + 4.0 * s;
    }

    // ---- content card (drawn after the tabs so it covers the selected tab's bottom edge)
    g.round_rect(geo.content, 8.0 * s, p.card, p.border, 1.0);
    if let Some(sel) = rects.get(d.tab) {
        // open the border under the selected tab and underline it
        g.fill_rect(RectF::new(sel.x + 1.0, geo.content.y - 1.0, sel.w - 2.0, 3.0 * s), p.card);
        g.line(sel.x + 14.0 * s, sel.bottom() - 2.0 * s, sel.right() - 14.0 * s, sel.bottom() - 2.0 * s, p.accent, 3.0 * s);
    }

    match (d.tab, d.snap) {
        (TAB_PROFILE, Some(sn)) => container_panel(g, &geo, &f, &p, &sn.profile, &mut hovers),
        (TAB_ODFC, Some(sn)) => container_panel(g, &geo, &f, &p, &sn.odfc, &mut hovers),
        _ => {} // Session Details / Report are native child controls inside geo.inner
    }
    PaintOut { tabs: rects, hovers }
}

fn short_user(u: &str) -> String {
    u.rsplit('\\').next().unwrap_or(u).to_string()
}

fn overall_value(sn: &Snapshot) -> String {
    match sn.light {
        Light::Grey => "Inactive".into(),
        Light::Red => "Error".into(),
        Light::Yellow => "Needs attention".into(),
        Light::Green => {
            let p = sn.profile.relevant && sn.profile.light == Light::Green;
            let o = sn.odfc.relevant && sn.odfc.light == Light::Green;
            match (p, o) {
                (true, true) => "Profile + Office attached".into(),
                (false, true) => "Office attached".into(),
                _ => "Profile attached".into(),
            }
        }
    }
}

enum Val {
    Text(String),
    Path(String),
    Badge(String, Light),
}

fn vhd_path(c: &Container) -> Option<&str> {
    c.values
        .iter()
        .filter(|(_, v)| {
            let l = v.trim().to_ascii_lowercase();
            l.ends_with(".vhdx") || l.ends_with(".vhd")
        })
        .max_by_key(|(n, _)| n.to_ascii_lowercase().contains("opened"))
        .map(|(_, v)| v.trim())
}

fn container_rows(c: &Container) -> Vec<(&'static str, Val)> {
    let mut rows = vec![("State", Val::Badge(c.summary.clone(), c.light))];
    rows.push((
        "Enabled",
        Val::Text(match c.enabled {
            Some(1) => "Yes".into(),
            Some(0) => "No".into(),
            Some(v) => v.to_string(),
            None => "Not set".into(),
        }),
    ));
    match &c.session_path {
        None => rows.push(("Session key", Val::Text("Not found for this user".into()))),
        Some(path) => {
            if let Some(st) = c.status {
                rows.push(("Status", Val::Text(format!("{} ({st})", status::status_text(st)))));
            }
            if let Some(r) = c.reason {
                rows.push(("Reason", Val::Text(format!("{} ({r})", status::reason_text(r)))));
            }
            if let Some(e) = c.error {
                let v = if e == 0 {
                    "None".to_string()
                } else {
                    format!("0x{e:08X} \u{2013} {}", win::error_text(e))
                };
                rows.push(("Error", Val::Text(v)));
            }
            rows.push(("Session key", Val::Path(path.clone())));
        }
    }
    if let Some(v) = vhd_path(c) {
        rows.push(("Container path", Val::Path(v.to_string())));
        let ty = if v.to_ascii_lowercase().ends_with(".vhdx") { "VHDX" } else { "VHD" };
        rows.push(("Type", Val::Text(ty.into())));
    }
    if let Some(v) = &c.volume {
        rows.push(("Access mode", Val::Text(if v.read_only { "Read-only" } else { "Read/Write" }.into())));
        rows.push(("Size (total)", Val::Text(win::fmt_bytes(v.total))));
        let pct = if v.total > 0 { v.free as f64 * 100.0 / v.total as f64 } else { 0.0 };
        rows.push(("Size (free)", Val::Text(format!("{} ({pct:.0}%)", win::fmt_bytes(v.free)))));
        if !v.fs.is_empty() {
            rows.push(("File system", Val::Text(v.fs.clone())));
        }
    }
    rows
}

fn container_panel(g: &Graphics, geo: &Geo, f: &Fonts, p: &Palette, c: &Container, hovers: &mut Vec<(RectF, String)>) {
    let s = geo.s;
    let card = geo.content;
    g.clip(card);

    // header
    let hy = card.y + 14.0 * s;
    status_glyph(g, c.light, card.x + 30.0 * s, hy + 13.0 * s, 11.0 * s);
    g.text(c.name, &f.section, RectF::new(card.x + 50.0 * s, hy, card.w - 70.0 * s, 26.0 * s), p.text, Align::Near, Trim::End);
    let sep_y = hy + 40.0 * s;
    g.line(card.x + 1.0, sep_y, card.right() - 1.0, sep_y, p.border, 1.0);

    let top = sep_y + 14.0 * s;
    let left = card.x + 20.0 * s;
    let mut grid_right = card.right() - 20.0 * s;

    // free-space donut
    if let Some(v) = &c.volume {
        let panel_w = 230.0 * s;
        if card.w > 560.0 * s {
            let px = card.right() - panel_w;
            grid_right = px - 16.0 * s;
            g.line(px, sep_y, px, card.bottom() - 1.0, p.border, 1.0);
            donut(g, geo, f, p, v, px, top, panel_w);
        }
    }

    // details grid
    let label_w = 140.0 * s;
    let row_h = 28.0 * s;
    let mut y = top;
    for (label, val) in container_rows(c) {
        if y + row_h > card.bottom() - 6.0 * s {
            break;
        }
        g.text(label, &f.grid, RectF::new(left, y, label_w - 8.0 * s, row_h), p.text2, Align::Near, Trim::End);
        let vx = left + label_w;
        let vw = (grid_right - vx).max(0.0);
        match val {
            Val::Text(t) => {
                let r = RectF::new(vx, y, vw, row_h);
                g.text(&t, &f.grid, r, p.text, Align::Near, Trim::End);
                hover_if_cut(g, hovers, &t, &f.grid, r);
            }
            Val::Path(t) => {
                let r = RectF::new(vx, y, vw, row_h);
                g.text(&t, &f.grid, r, p.text, Align::Near, Trim::Path);
                hover_if_cut(g, hovers, &t, &f.grid, r);
            }
            Val::Badge(t, l) => {
                let tw = g.measure(&t, &f.badge);
                let bw = (tw + 22.0 * s).min(vw);
                let b = RectF::new(vx - 2.0 * s, y + 3.0 * s, bw, row_h - 6.0 * s);
                g.round_rect(b, b.h / 2.0, p.status_tint(l), 0, 0.0);
                let tr = b.inset(11.0 * s, 0.0);
                g.text(&t, &f.badge, tr, p.status_text(l), Align::Center, Trim::End);
                if tw > tr.w + 0.5 {
                    hovers.push((b, t.clone()));
                }
            }
        }
        y += row_h;
    }
    g.reset_clip();
}

fn donut(g: &Graphics, geo: &Geo, f: &Fonts, p: &Palette, v: &win::VolumeInfo, px: f32, top: f32, pw: f32) {
    let s = geo.s;
    let r = 66.0 * s;
    let thick = 18.0 * s;
    let cx = px + pw / 2.0;
    let cy = top + 8.0 * s + r;
    let free_frac = if v.total > 0 { (v.free as f64 / v.total as f64).clamp(0.0, 1.0) as f32 } else { 0.0 };
    g.circle(cx, cy, r, p.track, thick);
    let free_sweep = 360.0 * free_frac;
    g.arc(cx, cy, r, -90.0, free_sweep, crate::theme::GREEN, thick);
    g.arc(cx, cy, r, -90.0 + free_sweep, 360.0 - free_sweep, p.used, thick);
    g.text(
        &format!("{:.0}%", free_frac * 100.0),
        &f.big,
        RectF::new(cx - r, cy - 24.0 * s, 2.0 * r, 34.0 * s),
        p.text,
        Align::Center,
        Trim::End,
    );
    g.text("Free", &f.grid, RectF::new(cx - r, cy + 10.0 * s, 2.0 * r, 20.0 * s), p.status_text(Light::Green), Align::Center, Trim::End);

    // legend
    let lx = px + 28.0 * s;
    let lw = pw - 40.0 * s;
    let mut ly = cy + r + thick / 2.0 + 18.0 * s;
    let used = v.total.saturating_sub(v.free);
    for (color, text) in [
        (crate::theme::GREEN, format!("Free:  {}", win::fmt_bytes(v.free))),
        (p.used, format!("Used:  {}", win::fmt_bytes(used))),
    ] {
        g.round_rect(RectF::new(lx, ly + 6.0 * s, 12.0 * s, 12.0 * s), 2.0 * s, color, 0, 0.0);
        g.text(&text, &f.grid, RectF::new(lx + 20.0 * s, ly, lw - 20.0 * s, 24.0 * s), p.text, Align::Near, Trim::End);
        ly += 26.0 * s;
    }
    g.line(lx, ly + 4.0 * s, lx + lw - 10.0 * s, ly + 4.0 * s, p.border, 1.0);
    g.text(
        &format!("Total:  {}", win::fmt_bytes(v.total)),
        &f.grid,
        RectF::new(lx, ly + 10.0 * s, lw, 24.0 * s),
        p.text,
        Align::Near,
        Trim::End,
    );
}

/// Owner-drawn button face.
pub fn button(g: &Graphics, w: f32, h: f32, dpi: u32, label: &str, primary: bool, pressed: bool, focused: bool, p: &Palette) {
    let s = dpi as f32 / 96.0;
    g.clear(p.window);
    let r = RectF::new(0.5, 0.5, w - 1.0, h - 1.0);
    let (fill, border, text) = if primary {
        (if pressed { p.accent_pressed } else { p.accent }, 0, p.accent_text)
    } else {
        let base = p.card;
        (if pressed { mix(p.text, base, 0.08) } else { base }, p.border, p.text)
    };
    g.round_rect(r, 5.0 * s, fill, border, 1.0);
    if focused {
        g.round_rect(r.inset(2.0 * s, 2.0 * s), 4.0 * s, 0, if primary { p.accent_text } else { p.accent }, 1.5 * s);
    }
    let font = Font::new(&[if primary { "Segoe UI Semibold" } else { "Segoe UI" }], 13.0 * s, false);
    let clean = label.replace('&', "");
    g.text(&clean, &font, r, text, Align::Center, Trim::End);
}

// ------------------------------------------------------------------ icons

pub fn status_glyph(g: &Graphics, l: Light, cx: f32, cy: f32, r: f32) {
    let col = match l {
        Light::Green => crate::theme::GREEN,
        Light::Yellow => crate::theme::AMBER,
        Light::Red => crate::theme::RED,
        Light::Grey => crate::theme::GREY,
    };
    let white = 0xFFFF_FFFF;
    let w = 0.2 * r;
    match l {
        Light::Grey => g.circle(cx, cy, r - w / 2.0, col, w),
        Light::Green => {
            g.fill_circle(cx, cy, r, col);
            g.line(cx - 0.45 * r, cy + 0.02 * r, cx - 0.12 * r, cy + 0.34 * r, white, w);
            g.line(cx - 0.12 * r, cy + 0.34 * r, cx + 0.46 * r, cy - 0.3 * r, white, w);
        }
        Light::Yellow => {
            g.fill_circle(cx, cy, r, col);
            g.line(cx, cy - 0.48 * r, cx, cy + 0.1 * r, white, w);
            g.fill_circle(cx, cy + 0.42 * r, 0.12 * r, white);
        }
        Light::Red => {
            g.fill_circle(cx, cy, r, col);
            g.line(cx - 0.45 * r, cy, cx + 0.45 * r, cy, white, w);
        }
    }
}

fn icon_user(g: &Graphics, cx: f32, cy: f32, sz: f32, col: u32) {
    g.fill_circle(cx, cy - 0.2 * sz, 0.18 * sz, col);
    g.pie(RectF::new(cx - 0.34 * sz, cy + 0.04 * sz, 0.68 * sz, 0.64 * sz), 180.0, 180.0, col);
}

fn icon_users(g: &Graphics, cx: f32, cy: f32, sz: f32, col: u32, bg: u32) {
    let back = mix(col, bg, 0.55);
    icon_user(g, cx + 0.2 * sz, cy - 0.04 * sz, sz * 0.8, back);
    icon_user(g, cx - 0.12 * sz, cy + 0.04 * sz, sz * 0.9, col);
}

fn icon_monitor(g: &Graphics, cx: f32, cy: f32, sz: f32, col: u32, bg: u32) {
    let screen = RectF::new(cx - 0.5 * sz, cy - 0.42 * sz, sz, 0.64 * sz);
    g.round_rect(screen, 0.08 * sz, col, 0, 0.0);
    g.round_rect(screen.inset(0.08 * sz, 0.08 * sz), 0.03 * sz, mix(bg, col, 0.25), 0, 0.0);
    g.line(cx, screen.bottom(), cx, cy + 0.36 * sz, col, 0.09 * sz);
    g.line(cx - 0.22 * sz, cy + 0.38 * sz, cx + 0.22 * sz, cy + 0.38 * sz, col, 0.09 * sz);
}

fn icon_briefcase(g: &Graphics, cx: f32, cy: f32, sz: f32, col: u32) {
    g.round_rect(RectF::new(cx - 0.17 * sz, cy - 0.4 * sz, 0.34 * sz, 0.24 * sz), 0.06 * sz, 0, col, 0.08 * sz);
    g.round_rect(RectF::new(cx - 0.45 * sz, cy - 0.22 * sz, 0.9 * sz, 0.6 * sz), 0.09 * sz, col, 0, 0.0);
}

fn icon_info(g: &Graphics, cx: f32, cy: f32, sz: f32, col: u32) {
    let r = 0.44 * sz;
    g.circle(cx, cy, r, col, 0.08 * sz);
    g.fill_circle(cx, cy - 0.2 * sz, 0.055 * sz, col);
    g.line(cx, cy - 0.04 * sz, cx, cy + 0.24 * sz, col, 0.09 * sz);
}

fn icon_lines(g: &Graphics, cx: f32, cy: f32, sz: f32, col: u32) {
    for i in -1..=1 {
        let y = cy + i as f32 * 0.28 * sz;
        g.fill_circle(cx - 0.36 * sz, y, 0.06 * sz, col);
        g.line(cx - 0.18 * sz, y, cx + 0.42 * sz, y, col, 0.09 * sz);
    }
}

impl RectF {
    /// Split into top (fraction `t`) and bottom rows.
    pub fn split_rows(&self, t: f32) -> (RectF, RectF) {
        let h1 = self.h * t;
        (RectF::new(self.x, self.y, self.w, h1), RectF::new(self.x, self.y + h1, self.w, self.h - h1))
    }
}
