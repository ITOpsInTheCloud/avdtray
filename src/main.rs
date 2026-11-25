//! AVD Tray - a lightweight, open replacement for the retired FSLogix frxtray.exe.
//!
//! Usage: avdtray.exe [--interval <seconds>] [--show] [--no-notify]
//! Running it a second time in the same session opens the status window of the
//! instance that is already running.
//!
//! Keyboard: Ctrl+Tab / Ctrl+Shift+Tab (or Ctrl+PgDn / Ctrl+PgUp) switch tabs,
//! Esc closes the window. The "Report" tab holds the full plain-text report,
//! which is the screen-reader friendly view.

#![cfg_attr(windows, windows_subsystem = "windows")]
#![allow(clippy::too_many_arguments)]

mod ffi;
mod gdip;
mod icons;
mod status;
mod theme;
mod view;
mod win;

use ffi::*;
use gdip::{Graphics, RectF};
use status::{Light, Snapshot};
use std::cell::{Cell, RefCell};
use std::ptr::null;
use std::sync::Mutex;
use theme::{colorref, Palette};
use win::wide;

const APP_NAME: &str = "AVD Tray";
const CLASS_NAME: &str = "AVDTray.Window";

const WM_TRAY: u32 = WM_APP + 1;
const WM_REDIRECTS_DONE: u32 = WM_APP + 2;
const WM_SHOW_REQUEST: u32 = WM_APP + 3;
const TIMER_ID: usize = 1;
const TRAY_UID: u32 = 1;

// control and menu ids
const ID_EDIT: usize = 100;
const ID_REFRESH: usize = 101;
const ID_COPY: usize = 102;
const ID_REDIRECTS: usize = 103;
const ID_LOG_PROFILE: usize = 104;
const ID_LIST: usize = 105;
const ID_CLOSE: usize = IDCANCEL; // so Esc closes the window
const ID_SHOW: usize = 201;
const ID_LOG_ODFC: usize = 202;
const ID_LOGS_FOLDER: usize = 203;
const ID_EVENTS: usize = 204;
const ID_ABOUT: usize = 205;
const ID_EXIT: usize = 206;

const BUTTONS: [(usize, &str); 5] = [
    (ID_REFRESH, "&Refresh"),
    (ID_COPY, "&Copy report"),
    (ID_REDIRECTS, "Re&directs"),
    (ID_LOG_PROFILE, "Open &log"),
    (ID_CLOSE, "Close"),
];

struct Options {
    interval_s: u32,
    show: bool,
    notify: bool,
}

#[derive(Default)]
struct App {
    hwnd: Cell<HWND>,
    edit: Cell<HWND>,
    list: Cell<HWND>,
    buttons: RefCell<Vec<HWND>>,
    font_mono: Cell<HFONT>,
    font_ui: Cell<HFONT>,
    icons: Cell<[HICON; 4]>,
    icon_added: Cell<bool>,
    light: Cell<Option<Light>>,
    taskbar_created: Cell<u32>,
    notify: Cell<bool>,
    sid: RefCell<Option<String>>,
    body: RefCell<String>,
    extra: RefCell<String>,
    tip: RefCell<String>,
    // painted UI
    snapshot: RefCell<Option<Snapshot>>,
    generated: RefCell<String>,
    tab: Cell<usize>,
    tab_rects: RefCell<Vec<RectF>>,
    pal: Cell<Palette>,
    ctl_brush: Cell<HBRUSH>,
    // tooltips for cut-off values
    tooltip: Cell<HWND>,
    hovers: RefCell<Vec<(RectF, String)>>,
    tool_count: Cell<usize>,
}

thread_local! {
    static APP: App = App::default();
}

/// Result handed back from the frx.exe worker thread.
static REDIRECTS: Mutex<Option<String>> = Mutex::new(None);

fn parse_args() -> Options {
    let mut o = Options { interval_s: 10, show: false, notify: true };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.to_ascii_lowercase().trim_start_matches(['-', '/']) {
            "interval" | "i" => {
                if let Some(v) = args.next().and_then(|v| v.parse::<u32>().ok()) {
                    o.interval_s = v.clamp(2, 3600);
                }
            }
            "show" | "s" => o.show = true,
            "no-notify" | "nonotify" => o.notify = false,
            _ => {}
        }
    }
    o
}

fn main() {
    let opts = parse_args();
    let class = wide(CLASS_NAME);

    unsafe {
        // One instance per user session. A second launch just opens the existing window.
        let mutex_name = wide(r"Local\AVDTray.SingleInstance");
        let _mutex = CreateMutexW(null(), 0, mutex_name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let other = FindWindowW(class.as_ptr(), null());
            if other != 0 {
                PostMessageW(other, WM_SHOW_REQUEST, 0, 0);
            }
            return;
        }

        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_TAB_CLASSES,
        };
        InitCommonControlsEx(&icc);
        // Kept alive until the end of main. If GDI+ is unavailable the tray still works.
        let _gdiplus = gdip::Gdiplus::start();

        let hinst = GetModuleHandleW(null());
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: 0,
            hCursor: LoadCursorW(0, IDC_ARROW as *const u16),
            hbrBackground: 0, // fully painted in WM_PAINT
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
            hIconSm: 0,
        };
        RegisterClassExW(&wc);

        let tb = wide("TaskbarCreated");
        APP.with(|a| {
            a.taskbar_created.set(RegisterWindowMessageW(tb.as_ptr()));
            a.notify.set(opts.notify);
            a.pal.set(Palette::detect());
            *a.sid.borrow_mut() = win::current_user_sid();
        });

        let title = wide(&format!("{APP_NAME} - FSLogix status"));
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            820,
            820,
            0,
            0,
            hinst,
            null(),
        );
        if hwnd == 0 {
            return;
        }
        // Size the (hidden) window for the monitor's DPI.
        let dpi = win::dpi_for_window(hwnd) as i32;
        SetWindowPos(hwnd, 0, 0, 0, 820 * dpi / 96, 820 * dpi / 96, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
        apply_theme(hwnd);

        build_icons(hwnd);
        refresh(true);
        SetTimer(hwnd, TIMER_ID, opts.interval_s * 1000, null());
        if opts.show {
            show_window();
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            // Ctrl+Tab / Ctrl+Shift+Tab / Ctrl+PgDn / Ctrl+PgUp switch tabs.
            if msg.message == WM_KEYDOWN
                && GetKeyState(VK_CONTROL) < 0
                && matches!(msg.wParam, VK_TAB | VK_NEXT | VK_PRIOR)
            {
                let back = msg.wParam == VK_PRIOR || (msg.wParam == VK_TAB && GetKeyState(VK_SHIFT) < 0);
                cycle_tab(back);
                continue;
            }
            if IsDialogMessageW(hwnd, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let icons = APP.with(|a| a.icons.get());
        for i in icons {
            if i != 0 {
                DestroyIcon(i);
            }
        }
    }
}

// ------------------------------------------------------------------ tray

fn build_icons(_hwnd: HWND) {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    let new = [
        icons::make(Light::Grey, size),
        icons::make(Light::Green, size),
        icons::make(Light::Yellow, size),
        icons::make(Light::Red, size),
    ];
    let old = APP.with(|a| a.icons.replace(new));
    for i in old {
        if i != 0 {
            unsafe { DestroyIcon(i) };
        }
    }
}

fn tray(action: u32, balloon: Option<(&str, &str, u32)>) -> bool {
    let (hwnd, icon, tip) = APP.with(|a| {
        let light = a.light.get().unwrap_or(Light::Grey);
        (a.hwnd.get(), a.icons.get()[light as usize], a.tip.borrow().clone())
    });
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_UID;
        if action != NIM_DELETE {
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY;
            nid.hIcon = icon;
            win::copy_fixed(&mut nid.szTip, &tip);
            if let Some((title, text, flags)) = balloon {
                nid.uFlags |= NIF_INFO;
                win::copy_fixed(&mut nid.szInfoTitle, title);
                win::copy_fixed(&mut nid.szInfo, text);
                nid.dwInfoFlags = flags;
            }
        }
        Shell_NotifyIconW(action, &nid) != 0
    }
}

/// Add the icon if the shell is ready; otherwise update it.
fn sync_tray(balloon: Option<(&str, &str, u32)>) {
    let added = APP.with(|a| a.icon_added.get());
    if added {
        if !tray(NIM_MODIFY, balloon) {
            // Explorer restarted without us noticing - re-add.
            let ok = tray(NIM_ADD, balloon);
            APP.with(|a| a.icon_added.set(ok));
        }
    } else {
        // At logon the taskbar may not exist yet; retried on every timer tick.
        let ok = tray(NIM_ADD, balloon);
        APP.with(|a| a.icon_added.set(ok));
    }
}

// ------------------------------------------------------------------ state

fn refresh(force_text: bool) {
    let sid = APP.with(|a| a.sid.borrow().clone());
    let snap: Snapshot = status::collect(sid.as_deref());
    let body = status::report_body(&snap);
    let tip = status::tooltip(&snap);
    let light = snap.light;
    let headline = snap.headline.clone();

    let (prev, changed, notify, hwnd) = APP.with(|a| {
        let prev = a.light.replace(Some(light));
        *a.tip.borrow_mut() = tip;
        let changed = *a.body.borrow() != body;
        if changed {
            *a.body.borrow_mut() = body;
        }
        *a.snapshot.borrow_mut() = Some(snap);
        *a.generated.borrow_mut() = win::now_string();
        (prev, changed, a.notify.get(), a.hwnd.get())
    });

    let balloon = if notify && light == Light::Red && prev != Some(Light::Red) {
        Some(("FSLogix problem detected", headline.as_str(), NIIF_ERROR))
    } else {
        None
    };
    sync_tray(balloon);

    if changed || force_text {
        update_text();
    }
    unsafe {
        if hwnd != 0 && IsWindowVisible(hwnd) != 0 {
            InvalidateRect(hwnd, null(), 0);
        }
    }
}

fn full_report() -> String {
    APP.with(|a| status::report(&a.body.borrow(), &a.extra.borrow()))
}

/// Push the latest data into the native child controls (report text + details list).
fn update_text() {
    let (hwnd, edit, list) = APP.with(|a| (a.hwnd.get(), a.edit.get(), a.list.get()));
    if edit == 0 || unsafe { IsWindowVisible(hwnd) } == 0 {
        return;
    }
    let text = wide(&full_report());
    unsafe {
        SetWindowTextW(edit, text.as_ptr());
    }
    let rows = APP.with(|a| a.snapshot.borrow().as_ref().map(detail_rows));
    if let Some(rows) = rows {
        fill_list(list, &rows);
    }
}

fn detail_rows(sn: &Snapshot) -> Vec<[String; 3]> {
    let mut rows: Vec<[String; 3]> = Vec::new();
    let mut push = |a: &str, b: &str, c: String| rows.push([a.to_string(), b.to_string(), c]);
    push("Session", "Computer", sn.computer.clone());
    push("Session", "User", sn.user.clone());
    push("Session", "SID", sn.sid.clone().unwrap_or_else(|| "<unknown>".into()));
    if let Some(id) = sn.session {
        push("Session", "Session ID", id.to_string());
    }
    push("Session", "Overall", format!("[{}] {}", sn.light.word(), sn.headline));
    for (sec, c) in [("Profile", &sn.profile), ("Office (ODFC)", &sn.odfc)] {
        push(sec, "State", format!("[{}] {}", c.light.word(), c.summary));
        push(sec, "Session key", c.session_path.clone().unwrap_or_else(|| "not found".into()));
        for (n, v) in &c.values {
            push(sec, n, v.clone());
        }
    }
    for comp in &sn.components {
        push(
            "Component",
            comp.name,
            format!(
                "{}  \u{00b7}  {}  \u{00b7}  {}",
                win::service_state_text(&comp.state),
                comp.version.as_deref().unwrap_or("-"),
                comp.display
            ),
        );
    }
    push("Component", "Install path", status::apps_dir().display().to_string());
    for sub in ["Profile", "ODFC"] {
        push(
            "Log",
            &format!("{sub} log"),
            status::newest_log(sub).map_or("none found".into(), |p| p.display().to_string()),
        );
    }
    rows
}

fn fill_list(list: HWND, rows: &[[String; 3]]) {
    if list == 0 {
        return;
    }
    unsafe {
        SendMessageW(list, LVM_DELETEALLITEMS, 0, 0);
        for (i, row) in rows.iter().enumerate() {
            let mut texts: Vec<Vec<u16>> = row.iter().map(|t| wide(t)).collect();
            let mut item: LVITEMW = std::mem::zeroed();
            item.mask = LVIF_TEXT;
            item.iItem = i as i32;
            item.iSubItem = 0;
            item.pszText = texts[0].as_mut_ptr();
            let idx = SendMessageW(list, LVM_INSERTITEMW, 0, &item as *const LVITEMW as LPARAM);
            if idx < 0 {
                continue;
            }
            for (sub, t) in texts.iter_mut().enumerate().skip(1) {
                let mut si: LVITEMW = std::mem::zeroed();
                si.iSubItem = sub as i32;
                si.pszText = t.as_mut_ptr();
                SendMessageW(list, LVM_SETITEMTEXTW, idx as WPARAM, &si as *const LVITEMW as LPARAM);
            }
        }
        for col in 0..3 {
            SendMessageW(list, LVM_SETCOLUMNWIDTH, col, LVSCW_AUTOSIZE_USEHEADER);
        }
    }
}

fn show_window() {
    let hwnd = APP.with(|a| a.hwnd.get());
    unsafe {
        ShowWindow(hwnd, if IsIconic(hwnd) != 0 { SW_RESTORE } else { SW_SHOW });
        SetForegroundWindow(hwnd);
    }
    refresh(true);
    sync_children();
}

// ------------------------------------------------------------------ tabs

fn set_tab(tab: usize) {
    let tab = tab % view::TAB_COUNT;
    let hwnd = APP.with(|a| {
        a.tab.set(tab);
        a.hwnd.get()
    });
    sync_children();
    unsafe {
        InvalidateRect(hwnd, null(), 0);
    }
}

fn cycle_tab(back: bool) {
    let t = APP.with(|a| a.tab.get());
    let n = view::TAB_COUNT;
    set_tab(if back { (t + n - 1) % n } else { (t + 1) % n });
}

/// Show only the native control that belongs to the current tab, and focus it.
fn sync_children() {
    let (tab, edit, list) = APP.with(|a| (a.tab.get(), a.edit.get(), a.list.get()));
    unsafe {
        ShowWindow(list, if tab == view::TAB_DETAILS { SW_SHOW } else { SW_HIDE });
        ShowWindow(edit, if tab == view::TAB_REPORT { SW_SHOW } else { SW_HIDE });
        match tab {
            view::TAB_DETAILS => {
                SetFocus(list);
            }
            view::TAB_REPORT => {
                SetFocus(edit);
            }
            _ => {
                let first = APP.with(|a| a.buttons.borrow().first().copied().unwrap_or(0));
                if first != 0 {
                    SetFocus(first);
                }
            }
        }
    }
}

// ------------------------------------------------------------------ window

fn apply_theme(hwnd: HWND) {
    let pal = Palette::detect();
    let (edit, list, tooltip, buttons, old_brush) = APP.with(|a| {
        a.pal.set(pal);
        (a.edit.get(), a.list.get(), a.tooltip.get(), a.buttons.borrow().clone(), a.ctl_brush.get())
    });
    unsafe {
        let dark: BOOL = pal.dark as BOOL;
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const BOOL as *const _, 4);
        let theme = wide(if pal.dark { "DarkMode_Explorer" } else { "Explorer" });
        for h in [edit, list, tooltip] {
            if h != 0 {
                SetWindowTheme(h, theme.as_ptr(), null());
            }
        }
        if list != 0 {
            SendMessageW(list, LVM_SETBKCOLOR, 0, colorref(pal.card) as LPARAM);
            SendMessageW(list, LVM_SETTEXTBKCOLOR, 0, colorref(pal.card) as LPARAM);
            SendMessageW(list, LVM_SETTEXTCOLOR, 0, colorref(pal.text) as LPARAM);
        }
        let brush = CreateSolidBrush(colorref(pal.card));
        APP.with(|a| a.ctl_brush.set(brush));
        if old_brush != 0 {
            DeleteObject(old_brush);
        }
        InvalidateRect(hwnd, null(), 0);
        for h in buttons.into_iter().chain([edit, list]) {
            if h != 0 {
                InvalidateRect(h, null(), 1);
            }
        }
    }
}

fn make_fonts(hwnd: HWND) {
    let dpi = win::dpi_for_window(hwnd) as i32;
    let height = -(9 * dpi / 72);
    let mono = wide("Consolas");
    let ui = wide("Segoe UI");
    let (fm, fu) = unsafe {
        (
            CreateFontW(height, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, FIXED_PITCH | FF_MODERN, mono.as_ptr()),
            CreateFontW(height, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, VARIABLE_PITCH | FF_SWISS, ui.as_ptr()),
        )
    };
    let (old_m, old_u, edit, list, buttons) = APP.with(|a| {
        (a.font_mono.replace(fm), a.font_ui.replace(fu), a.edit.get(), a.list.get(), a.buttons.borrow().clone())
    });
    unsafe {
        if edit != 0 {
            SendMessageW(edit, WM_SETFONT, fm as usize, 1);
        }
        if list != 0 {
            SendMessageW(list, WM_SETFONT, fu as usize, 1);
        }
        for b in buttons {
            SendMessageW(b, WM_SETFONT, fu as usize, 1);
        }
        if old_m != 0 {
            DeleteObject(old_m);
        }
        if old_u != 0 {
            DeleteObject(old_u);
        }
    }
}

fn create_controls(hwnd: HWND) {
    let hinst = unsafe { GetModuleHandleW(null()) };
    let edit_class = wide("EDIT");
    let list_class = wide("SysListView32");
    let button_class = wide("BUTTON");
    let empty = wide("");
    unsafe {
        let edit = CreateWindowExW(
            0,
            edit_class.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | ES_AUTOHSCROLL | ES_NOHIDESEL,
            0,
            0,
            100,
            100,
            hwnd,
            ID_EDIT as HMENU,
            hinst,
            null(),
        );
        SendMessageW(edit, EM_SETLIMITTEXT, 0, 0); // remove 32K limit

        let list = CreateWindowExW(
            0,
            list_class.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_TABSTOP | LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS | LVS_NOSORTHEADER,
            0,
            0,
            100,
            100,
            hwnd,
            ID_LIST as HMENU,
            hinst,
            null(),
        );
        let ex = LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER;
        SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, ex as WPARAM, ex);
        for (i, (title, width)) in [("Section", 110), ("Name", 200), ("Value", 400)].iter().enumerate() {
            let mut t = wide(title);
            let mut col: LVCOLUMNW = std::mem::zeroed();
            col.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
            col.cx = *width;
            col.pszText = t.as_mut_ptr();
            col.iSubItem = i as i32;
            SendMessageW(list, LVM_INSERTCOLUMNW, i, &col as *const LVCOLUMNW as LPARAM);
        }

        let mut buttons = Vec::new();
        for (id, label) in BUTTONS {
            let l = wide(label);
            let b = CreateWindowExW(
                0,
                button_class.as_ptr(),
                l.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW,
                0,
                0,
                10,
                10,
                hwnd,
                id as HMENU,
                hinst,
                null(),
            );
            buttons.push(b);
        }
        let tip_class = wide("tooltips_class32");
        let tooltip = CreateWindowExW(
            WS_EX_TOPMOST,
            tip_class.as_ptr(),
            null(),
            WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            hwnd,
            0,
            hinst,
            null(),
        );
        if tooltip != 0 {
            let dpi = win::dpi_for_window(hwnd) as isize;
            SendMessageW(tooltip, TTM_SETMAXTIPWIDTH, 0, 700 * dpi / 96); // wrap very long values
            SendMessageW(tooltip, TTM_SETDELAYTIME, TTDT_AUTOPOP, 30_000); // keep it up long enough to read
        }
        APP.with(|a| {
            a.hwnd.set(hwnd);
            a.edit.set(edit);
            a.list.set(list);
            a.tooltip.set(tooltip);
            *a.buttons.borrow_mut() = buttons;
        });
    }
    make_fonts(hwnd);
}

fn client_size(hwnd: HWND) -> (i32, i32) {
    let mut rc = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rc) };
    (rc.right, rc.bottom)
}

fn layout(hwnd: HWND) {
    let (edit, list, buttons) = APP.with(|a| (a.edit.get(), a.list.get(), a.buttons.borrow().clone()));
    if edit == 0 {
        return;
    }
    let dpi = win::dpi_for_window(hwnd);
    let (w, h) = client_size(hwnd);
    let geo = view::geometry(w as f32, h as f32, dpi);
    let r = geo.inner;
    unsafe {
        for c in [edit, list] {
            MoveWindow(c, r.x as i32, r.y as i32, r.w as i32, r.h as i32, 1);
        }
        let bw = geo.button_w as i32;
        let bh = geo.button_h as i32;
        let y = geo.buttons_y as i32;
        let gap = (8.0 * geo.s) as i32;
        let n = buttons.len();
        for (i, b) in buttons.iter().enumerate() {
            // last button (Close) is right-aligned, the rest left-aligned
            let x = if i == n - 1 { w - geo.pad as i32 - bw } else { geo.pad as i32 + i as i32 * (bw + gap) };
            MoveWindow(*b, x, y, bw, bh, 1);
        }
        InvalidateRect(hwnd, null(), 0);
    }
}

/// Double-buffered GDI+ painting into `hdc`.
fn paint_buffered(hdc: HDC, w: i32, h: i32, draw: impl FnOnce(&Graphics)) {
    if w <= 0 || h <= 0 {
        return;
    }
    unsafe {
        let mem = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, w, h);
        let old = SelectObject(mem, bmp);
        if let Some(g) = Graphics::from_hdc(mem) {
            draw(&g);
            // g is dropped here, which flushes GDI+ before the blit
        }
        BitBlt(hdc, 0, 0, w, h, mem, 0, 0, SRCCOPY);
        SelectObject(mem, old);
        DeleteObject(bmp);
        DeleteDC(mem);
    }
}

fn on_paint(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        let (w, h) = client_size(hwnd);
        let dpi = win::dpi_for_window(hwnd);
        let mut out = view::PaintOut::default();
        paint_buffered(hdc, w, h, |g| {
            out = APP.with(|a| {
                let snap = a.snapshot.borrow();
                let generated = a.generated.borrow();
                let data = view::ViewData { snap: snap.as_ref(), generated: &generated, tab: a.tab.get(), pal: a.pal.get() };
                view::paint(g, w as f32, h as f32, dpi, &data)
            });
        });
        APP.with(|a| *a.tab_rects.borrow_mut() = out.tabs);
        EndPaint(hwnd, &ps);
        sync_tooltips(hwnd, out.hovers);
    }
}

/// One tooltip area per cut-off value. Only rebuilt when the areas or texts change,
/// so the 10-second refresh doesn't close a tooltip the user is reading.
fn sync_tooltips(hwnd: HWND, hovers: Vec<(RectF, String)>) {
    let (tooltip, old_count, same) = APP.with(|a| (a.tooltip.get(), a.tool_count.get(), *a.hovers.borrow() == hovers));
    if tooltip == 0 || same {
        return;
    }
    unsafe {
        for id in 1..=old_count {
            let mut ti: TTTOOLINFOW = std::mem::zeroed();
            ti.cbSize = TTTOOLINFOW_V2_SIZE;
            ti.hwnd = hwnd;
            ti.uId = id;
            SendMessageW(tooltip, TTM_DELTOOLW, 0, &ti as *const TTTOOLINFOW as LPARAM);
        }
        let hinst = GetModuleHandleW(null());
        for (i, (r, text)) in hovers.iter().enumerate() {
            let mut t = wide(text);
            let mut ti: TTTOOLINFOW = std::mem::zeroed();
            ti.cbSize = TTTOOLINFOW_V2_SIZE;
            ti.uFlags = TTF_SUBCLASS;
            ti.hwnd = hwnd;
            ti.uId = i + 1;
            ti.rect = RECT {
                left: r.x as i32,
                top: r.y as i32,
                right: r.right().ceil() as i32,
                bottom: r.bottom().ceil() as i32,
            };
            ti.hinst = hinst;
            ti.lpszText = t.as_mut_ptr(); // the tooltip copies the text
            SendMessageW(tooltip, TTM_ADDTOOLW, 0, &ti as *const TTTOOLINFOW as LPARAM);
        }
    }
    APP.with(|a| {
        a.tool_count.set(hovers.len());
        *a.hovers.borrow_mut() = hovers;
    });
}

fn on_draw_item(hwnd: HWND, di: &DRAWITEMSTRUCT) {
    let id = di.CtlID as usize;
    let label = BUTTONS.iter().find(|(i, _)| *i == id).map_or("", |(_, l)| l);
    let pal = APP.with(|a| a.pal.get());
    let w = di.rcItem.right - di.rcItem.left;
    let h = di.rcItem.bottom - di.rcItem.top;
    let dpi = win::dpi_for_window(hwnd);
    paint_buffered(di.hDC, w, h, |g| {
        view::button(
            g,
            w as f32,
            h as f32,
            dpi,
            label,
            id == ID_REFRESH,
            di.itemState & ODS_SELECTED != 0,
            di.itemState & ODS_FOCUS != 0 && di.itemState & ODS_NOFOCUSRECT == 0,
            &pal,
        );
    });
}

fn tab_at(x: f32, y: f32) -> Option<usize> {
    APP.with(|a| a.tab_rects.borrow().iter().position(|r| r.contains(x, y)))
}

fn show_menu(hwnd: HWND) {
    let has_profile_log = status::newest_log("Profile").is_some();
    let has_odfc_log = status::newest_log("ODFC").is_some();
    unsafe {
        let m = CreatePopupMenu();
        let add = |id: usize, text: &str, enabled: bool| {
            let t = wide(text);
            AppendMenuW(m, MF_STRING | if enabled { 0 } else { MF_GRAYED }, id, t.as_ptr());
        };
        let sep = || {
            AppendMenuW(m, MF_SEPARATOR, 0, null());
        };
        add(ID_SHOW, "&Show status…", true);
        add(ID_REFRESH, "&Refresh now", true);
        add(ID_COPY, "&Copy report to clipboard", true);
        sep();
        add(ID_LOG_PROFILE, "Open latest &Profile log", has_profile_log);
        add(ID_LOG_ODFC, "Open latest &Office (ODFC) log", has_odfc_log);
        add(ID_LOGS_FOLDER, "Open logs &folder", true);
        add(ID_EVENTS, "Open FSLogix &event log", true);
        add(ID_REDIRECTS, "List re&directs (frx.exe)", true);
        sep();
        add(ID_ABOUT, "&About", true);
        add(ID_EXIT, "E&xit", true);
        SetMenuDefaultItem(m, ID_SHOW as u32, 0);

        let mut pt = POINT::default();
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd); // required so the menu closes when clicking elsewhere
        let cmd = TrackPopupMenu(m, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(m);
        if cmd > 0 {
            command(hwnd, cmd as usize);
        }
    }
}

fn open_log(hwnd: HWND, sub: &str) {
    match status::newest_log(sub) {
        Some(p) => {
            win::shell_open(&p.to_string_lossy(), None);
        }
        None => win::message_box(
            hwnd,
            &format!("No {sub} log found in {}", status::logs_dir().join(sub).display()),
            APP_NAME,
            MB_OK | MB_ICONINFORMATION,
        ),
    }
}

fn start_redirects(hwnd: HWND) {
    APP.with(|a| *a.extra.borrow_mut() = "=== frx.exe list-redirects ===\r\nRunning…".into());
    show_window();
    set_tab(view::TAB_REPORT);
    std::thread::spawn(move || {
        let exe = status::apps_dir().join("frx.exe");
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg("list-redirects");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let text = match cmd.output() {
            Ok(o) => {
                let mut t = String::from_utf8_lossy(&o.stdout).into_owned();
                let err = String::from_utf8_lossy(&o.stderr);
                if !err.trim().is_empty() {
                    t.push_str("\n[stderr]\n");
                    t.push_str(&err);
                }
                if t.trim().is_empty() {
                    t = format!("(no output, exit code {:?})", o.status.code());
                }
                t
            }
            Err(e) => format!("Could not run {}: {e}", exe.display()),
        };
        let text = text.replace("\r\n", "\n").replace('\n', "\r\n");
        if let Ok(mut slot) = REDIRECTS.lock() {
            *slot = Some(format!("=== frx.exe list-redirects ===\r\n{}", text.trim_end()));
        }
        unsafe {
            PostMessageW(hwnd, WM_REDIRECTS_DONE, 0, 0);
        }
    });
}

fn command(hwnd: HWND, id: usize) {
    match id {
        ID_SHOW => show_window(),
        ID_REFRESH => refresh(true),
        ID_COPY => {
            refresh(false);
            if !win::set_clipboard(hwnd, &full_report()) {
                win::message_box(hwnd, "Could not open the clipboard.", APP_NAME, MB_OK | MB_ICONWARNING);
            }
        }
        ID_REDIRECTS => start_redirects(hwnd),
        ID_LOG_PROFILE => open_log(hwnd, "Profile"),
        ID_LOG_ODFC => open_log(hwnd, "ODFC"),
        ID_LOGS_FOLDER => {
            win::shell_open(&status::logs_dir().to_string_lossy(), None);
        }
        ID_EVENTS => {
            win::shell_open("eventvwr.exe", Some("/c:Microsoft-FSLogix-Apps/Operational"));
        }
        ID_ABOUT => win::message_box(
            hwnd,
            &format!(
                "{APP_NAME} {}\n\nLightweight FSLogix profile status tray tool.\nA community replacement for the retired frxtray.exe.\n\nKeyboard: Ctrl+Tab switches tabs, Esc closes the window.\n\nNot affiliated with or supported by Microsoft.",
                env!("CARGO_PKG_VERSION")
            ),
            APP_NAME,
            MB_OK | MB_ICONINFORMATION,
        ),
        ID_CLOSE => unsafe {
            ShowWindow(hwnd, SW_HIDE);
        },
        ID_EXIT => unsafe {
            DestroyWindow(hwnd);
        },
        _ => {}
    }
}

fn lparam_xy(lp: LPARAM) -> (f32, f32) {
    ((lp & 0xFFFF) as u16 as i16 as f32, ((lp >> 16) & 0xFFFF) as u16 as i16 as f32)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            create_controls(hwnd);
            0
        }
        WM_SIZE => {
            layout(hwnd);
            0
        }
        WM_ERASEBKGND => 1, // everything is painted in WM_PAINT
        WM_PAINT => {
            on_paint(hwnd);
            0
        }
        WM_DRAWITEM => {
            if lp != 0 {
                on_draw_item(hwnd, &*(lp as *const DRAWITEMSTRUCT));
            }
            1
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            let (edit, pal, brush) = APP.with(|a| (a.edit.get(), a.pal.get(), a.ctl_brush.get()));
            if lp == edit && brush != 0 {
                SetTextColor(wp as HDC, colorref(pal.text));
                SetBkColor(wp as HDC, colorref(pal.card));
                return brush;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            if let Some(t) = tab_at(x, y) {
                set_tab(t);
            }
            0
        }
        WM_SETCURSOR => {
            if wp as HWND == hwnd && (lp & 0xFFFF) == HTCLIENT {
                let mut pt = POINT::default();
                GetCursorPos(&mut pt);
                ScreenToClient(hwnd, &mut pt);
                if tab_at(pt.x as f32, pt.y as f32).is_some() {
                    SetCursor(LoadCursorW(0, IDC_HAND as *const u16));
                    return 1;
                }
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_GETMINMAXINFO => {
            if lp != 0 {
                let dpi = win::dpi_for_window(hwnd) as i32;
                let mmi = &mut *(lp as *mut MINMAXINFO);
                mmi.ptMinTrackSize = POINT { x: 700 * dpi / 96, y: 620 * dpi / 96 };
            }
            0
        }
        WM_SETTINGCHANGE => {
            apply_theme(hwnd);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_DPICHANGED => {
            let r = &*(lp as *const RECT);
            make_fonts(hwnd);
            SetWindowPos(hwnd, 0, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
            layout(hwnd);
            0
        }
        WM_TIMER => {
            refresh(false);
            0
        }
        WM_TRAY => {
            match (lp as u32) & 0xFFFF {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => show_window(),
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            let id = wp & 0xFFFF;
            let code = (wp >> 16) & 0xFFFF;
            // ignore EN_* notifications from the edit control
            if lp != 0 && code != BN_CLICKED {
                return 0;
            }
            command(hwnd, id);
            0
        }
        WM_SHOW_REQUEST => {
            show_window();
            0
        }
        WM_REDIRECTS_DONE => {
            let text = REDIRECTS.lock().ok().and_then(|mut s| s.take());
            if let Some(t) = text {
                APP.with(|a| *a.extra.borrow_mut() = t);
            }
            update_text();
            0
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_DESTROY => {
            tray(NIM_DELETE, None);
            let (fm, fu, br) = APP.with(|a| (a.font_mono.replace(0), a.font_ui.replace(0), a.ctl_brush.replace(0)));
            for h in [fm, fu, br] {
                if h != 0 {
                    DeleteObject(h);
                }
            }
            PostQuitMessage(0);
            0
        }
        _ => {
            let tb = APP.with(|a| a.taskbar_created.get());
            if tb != 0 && msg == tb {
                // Explorer restarted: icons may need a new size, then re-add.
                build_icons(hwnd);
                APP.with(|a| a.icon_added.set(false));
                sync_tray(None);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
}
