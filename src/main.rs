//! AVD Tray - a lightweight, open replacement for the retired FSLogix frxtray.exe.
//!
//! Usage: avdtray.exe [--interval <seconds>] [--show] [--no-notify]
//! Running it a second time in the same session opens the status window of the
//! instance that is already running.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod ffi;
mod icons;
mod status;
mod win;

use ffi::*;
use status::{Light, Snapshot};
use std::cell::{Cell, RefCell};
use std::ptr::null;
use std::sync::Mutex;
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
            hbrBackground: COLOR_BTNFACE + 1,
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
            hIconSm: 0,
        };
        RegisterClassExW(&wc);

        let tb = wide("TaskbarCreated");
        APP.with(|a| {
            a.taskbar_created.set(RegisterWindowMessageW(tb.as_ptr()));
            a.notify.set(opts.notify);
            *a.sid.borrow_mut() = win::current_user_sid();
        });

        let title = wide(&format!("{APP_NAME} - FSLogix status"));
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            860,
            640,
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
        SetWindowPos(hwnd, 0, 0, 0, 860 * dpi / 96, 640 * dpi / 96, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);

        build_icons(hwnd);
        refresh(true);
        SetTimer(hwnd, TIMER_ID, opts.interval_s * 1000, null());
        if opts.show {
            show_window();
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
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

    let (prev, changed, notify) = APP.with(|a| {
        let prev = a.light.replace(Some(snap.light));
        *a.tip.borrow_mut() = tip;
        let changed = *a.body.borrow() != body;
        if changed {
            *a.body.borrow_mut() = body;
        }
        (prev, changed, a.notify.get())
    });

    let balloon_text;
    let balloon = if notify && snap.light == Light::Red && prev != Some(Light::Red) {
        balloon_text = snap.headline.clone();
        Some(("FSLogix problem detected", balloon_text.as_str(), NIIF_ERROR))
    } else {
        None
    };
    sync_tray(balloon);

    if changed || force_text {
        update_text();
    }
}

fn full_report() -> String {
    APP.with(|a| status::report(&a.body.borrow(), &a.extra.borrow()))
}

fn update_text() {
    let (hwnd, edit) = APP.with(|a| (a.hwnd.get(), a.edit.get()));
    if edit == 0 || unsafe { IsWindowVisible(hwnd) } == 0 {
        return;
    }
    let text = wide(&full_report());
    unsafe {
        SetWindowTextW(edit, text.as_ptr());
    }
}

fn show_window() {
    let hwnd = APP.with(|a| a.hwnd.get());
    unsafe {
        ShowWindow(hwnd, if IsIconic(hwnd) != 0 { SW_RESTORE } else { SW_SHOW });
        SetForegroundWindow(hwnd);
    }
    refresh(true);
    let edit = APP.with(|a| a.edit.get());
    unsafe {
        SetFocus(edit);
    }
}

// ------------------------------------------------------------------ window

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
    let (old_m, old_u, edit, buttons) = APP.with(|a| {
        (a.font_mono.replace(fm), a.font_ui.replace(fu), a.edit.get(), a.buttons.borrow().clone())
    });
    unsafe {
        if edit != 0 {
            SendMessageW(edit, WM_SETFONT, fm as usize, 1);
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
    let button_class = wide("BUTTON");
    let empty = wide("");
    unsafe {
        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_class.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | ES_AUTOHSCROLL | ES_NOHIDESEL,
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
        let mut buttons = Vec::new();
        for (id, label) in BUTTONS {
            let l = wide(label);
            let b = CreateWindowExW(
                0,
                button_class.as_ptr(),
                l.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON,
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
        APP.with(|a| {
            a.hwnd.set(hwnd);
            a.edit.set(edit);
            *a.buttons.borrow_mut() = buttons;
        });
    }
    make_fonts(hwnd);
}

fn layout(hwnd: HWND) {
    let (edit, buttons) = APP.with(|a| (a.edit.get(), a.buttons.borrow().clone()));
    if edit == 0 {
        return;
    }
    let dpi = win::dpi_for_window(hwnd) as i32;
    let sc = |v: i32| v * dpi / 96;
    let mut rc = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rc) };
    let (w, h) = (rc.right, rc.bottom);
    let m = sc(8);
    let bh = sc(28);
    let bw = sc(104);
    unsafe {
        MoveWindow(edit, m, m, (w - 2 * m).max(10), (h - 3 * m - bh).max(10), 1);
        let y = h - m - bh;
        let n = buttons.len() as i32;
        for (i, b) in buttons.iter().enumerate() {
            let i = i as i32;
            // last button (Close) is right-aligned, the rest left-aligned
            let x = if i == n - 1 { w - m - bw } else { m + i * (bw + sc(6)) };
            MoveWindow(*b, x, y, bw, bh, 1);
        }
    }
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
                "{APP_NAME} {}\n\nLightweight FSLogix profile status tray tool.\nA community replacement for the retired frxtray.exe.\n\nNot affiliated with or supported by Microsoft.",
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
            let (fm, fu) = APP.with(|a| (a.font_mono.replace(0), a.font_ui.replace(0)));
            if fm != 0 {
                DeleteObject(fm);
            }
            if fu != 0 {
                DeleteObject(fu);
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
