//! Minimal hand-written Win32 bindings. No external crates, so the project
//! builds offline and the binary stays tiny.
#![allow(non_snake_case, non_camel_case_types, dead_code, clippy::upper_case_acronyms)]

use core::ffi::c_void;

pub type HANDLE = isize;
pub type HWND = isize;
pub type HINSTANCE = isize;
pub type HICON = isize;
pub type HMENU = isize;
pub type HKEY = isize;
pub type HBITMAP = isize;
pub type HFONT = isize;
pub type HGDIOBJ = isize;
pub type HCURSOR = isize;
pub type HBRUSH = isize;
pub type HDC = isize;
pub type SC_HANDLE = isize;
pub type BOOL = i32;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type WNDPROC = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HICON,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cbSize: u32,
    pub hWnd: HWND,
    pub uID: u32,
    pub uFlags: u32,
    pub uCallbackMessage: u32,
    pub hIcon: HICON,
    pub szTip: [u16; 128],
    pub dwState: u32,
    pub dwStateMask: u32,
    pub szInfo: [u16; 256],
    pub uVersion: u32, // union with uTimeout
    pub szInfoTitle: [u16; 64],
    pub dwInfoFlags: u32,
    pub guidItem: GUID,
    pub hBalloonIcon: HICON,
}

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub biSize: u32,
    pub biWidth: i32,
    pub biHeight: i32,
    pub biPlanes: u16,
    pub biBitCount: u16,
    pub biCompression: u32,
    pub biSizeImage: u32,
    pub biXPelsPerMeter: i32,
    pub biYPelsPerMeter: i32,
    pub biClrUsed: u32,
    pub biClrImportant: u32,
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [u32; 1],
}

#[repr(C)]
pub struct ICONINFO {
    pub fIcon: BOOL,
    pub xHotspot: u32,
    pub yHotspot: u32,
    pub hbmMask: HBITMAP,
    pub hbmColor: HBITMAP,
}

#[repr(C)]
#[derive(Default)]
pub struct SERVICE_STATUS {
    pub dwServiceType: u32,
    pub dwCurrentState: u32,
    pub dwControlsAccepted: u32,
    pub dwWin32ExitCode: u32,
    pub dwServiceSpecificExitCode: u32,
    pub dwCheckPoint: u32,
    pub dwWaitHint: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VS_FIXEDFILEINFO {
    pub dwSignature: u32,
    pub dwStrucVersion: u32,
    pub dwFileVersionMS: u32,
    pub dwFileVersionLS: u32,
    pub dwProductVersionMS: u32,
    pub dwProductVersionLS: u32,
    pub dwFileFlagsMask: u32,
    pub dwFileFlags: u32,
    pub dwFileOS: u32,
    pub dwFileType: u32,
    pub dwFileSubtype: u32,
    pub dwFileDateMS: u32,
    pub dwFileDateLS: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SYSTEMTIME {
    pub wYear: u16,
    pub wMonth: u16,
    pub wDayOfWeek: u16,
    pub wDay: u16,
    pub wHour: u16,
    pub wMinute: u16,
    pub wSecond: u16,
    pub wMilliseconds: u16,
}

// ---------------------------------------------------------------- constants
// Predefined registry roots are sign-extended 32-bit values.
pub const HKEY_CURRENT_USER: HKEY = 0x8000_0001u32 as i32 as isize;
pub const HKEY_LOCAL_MACHINE: HKEY = 0x8000_0002u32 as i32 as isize;
pub const KEY_READ: u32 = 0x20019;
pub const KEY_WOW64_64KEY: u32 = 0x0100;
pub const REG_SZ: u32 = 1;
pub const REG_EXPAND_SZ: u32 = 2;
pub const REG_BINARY: u32 = 3;
pub const REG_DWORD: u32 = 4;
pub const REG_MULTI_SZ: u32 = 7;
pub const REG_QWORD: u32 = 11;
pub const ERROR_SUCCESS: i32 = 0;
pub const ERROR_MORE_DATA: i32 = 234;
pub const ERROR_NO_MORE_ITEMS: i32 = 259;
pub const ERROR_ALREADY_EXISTS: u32 = 183;

pub const WM_NULL: u32 = 0x0000;
pub const WM_CREATE: u32 = 0x0001;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_SIZE: u32 = 0x0005;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_CONTEXTMENU: u32 = 0x007B;
pub const WM_COMMAND: u32 = 0x0111;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_LBUTTONDBLCLK: u32 = 0x0203;
pub const WM_RBUTTONUP: u32 = 0x0205;
pub const WM_DPICHANGED: u32 = 0x02E0;
pub const WM_APP: u32 = 0x8000;

pub const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_VSCROLL: u32 = 0x0020_0000;
pub const WS_HSCROLL: u32 = 0x0010_0000;
pub const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;
pub const ES_MULTILINE: u32 = 0x0004;
pub const ES_AUTOVSCROLL: u32 = 0x0040;
pub const ES_AUTOHSCROLL: u32 = 0x0080;
pub const ES_NOHIDESEL: u32 = 0x0100;
pub const ES_READONLY: u32 = 0x0800;
pub const BS_PUSHBUTTON: u32 = 0x0000;
pub const EM_SETLIMITTEXT: u32 = 0x00C5;
pub const BN_CLICKED: usize = 0;
pub const IDCANCEL: usize = 2;
pub const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;
pub const COLOR_BTNFACE: isize = 15;
pub const IDC_ARROW: usize = 32512;

pub const SW_HIDE: i32 = 0;
pub const SW_SHOWNORMAL: i32 = 1;
pub const SW_SHOW: i32 = 5;
pub const SW_RESTORE: i32 = 9;
pub const SWP_NOSIZE: u32 = 0x0001;
pub const SWP_NOMOVE: u32 = 0x0002;
pub const SWP_NOZORDER: u32 = 0x0004;
pub const SWP_NOACTIVATE: u32 = 0x0010;

pub const MF_STRING: u32 = 0x0000;
pub const MF_GRAYED: u32 = 0x0001;
pub const MF_SEPARATOR: u32 = 0x0800;
pub const TPM_RIGHTBUTTON: u32 = 0x0002;
pub const TPM_NONOTIFY: u32 = 0x0080;
pub const TPM_RETURNCMD: u32 = 0x0100;

pub const SM_CXSMICON: i32 = 49;
pub const MB_OK: u32 = 0x0000;
pub const MB_ICONINFORMATION: u32 = 0x0040;
pub const MB_ICONWARNING: u32 = 0x0030;

pub const NIM_ADD: u32 = 0;
pub const NIM_MODIFY: u32 = 1;
pub const NIM_DELETE: u32 = 2;
pub const NIF_MESSAGE: u32 = 0x01;
pub const NIF_ICON: u32 = 0x02;
pub const NIF_TIP: u32 = 0x04;
pub const NIF_INFO: u32 = 0x10;
pub const NIIF_INFO: u32 = 1;
pub const NIIF_WARNING: u32 = 2;
pub const NIIF_ERROR: u32 = 3;

pub const GMEM_MOVEABLE: u32 = 0x0002;
pub const CF_UNICODETEXT: u32 = 13;
pub const TOKEN_QUERY: u32 = 0x0008;
pub const TOKEN_USER_CLASS: u32 = 1;
pub const SC_MANAGER_CONNECT: u32 = 0x0001;
pub const SERVICE_QUERY_STATUS: u32 = 0x0004;
pub const FILE_READ_ONLY_VOLUME: u32 = 0x0008_0000;
pub const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x0200;
pub const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x1000;
pub const DIB_RGB_COLORS: u32 = 0;
pub const BI_RGB: u32 = 0;

pub const FW_NORMAL: i32 = 400;
pub const DEFAULT_CHARSET: u32 = 1;
pub const CLEARTYPE_QUALITY: u32 = 5;
pub const FIXED_PITCH: u32 = 1;
pub const VARIABLE_PITCH: u32 = 2;
pub const FF_SWISS: u32 = 0x20;
pub const FF_MODERN: u32 = 0x30;

// ---------------------------------------------------------------- functions
#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(wc: *const WNDCLASSEXW) -> u16;
    pub fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: HWND,
        menu: HMENU,
        inst: HINSTANCE,
        param: *const c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT;
    pub fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: u32, max: u32) -> BOOL;
    pub fn TranslateMessage(msg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    pub fn IsDialogMessageW(hwnd: HWND, msg: *mut MSG) -> BOOL;
    pub fn PostQuitMessage(code: i32);
    pub fn DestroyWindow(hwnd: HWND) -> BOOL;
    pub fn ShowWindow(hwnd: HWND, cmd: i32) -> BOOL;
    pub fn IsWindowVisible(hwnd: HWND) -> BOOL;
    pub fn IsIconic(hwnd: HWND) -> BOOL;
    pub fn SetForegroundWindow(hwnd: HWND) -> BOOL;
    pub fn SetFocus(hwnd: HWND) -> HWND;
    pub fn FindWindowW(class: *const u16, name: *const u16) -> HWND;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn AppendMenuW(menu: HMENU, flags: u32, id: usize, text: *const u16) -> BOOL;
    pub fn SetMenuDefaultItem(menu: HMENU, item: u32, by_pos: u32) -> BOOL;
    pub fn TrackPopupMenu(
        menu: HMENU,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        hwnd: HWND,
        rect: *const RECT,
    ) -> BOOL;
    pub fn DestroyMenu(menu: HMENU) -> BOOL;
    pub fn GetCursorPos(pt: *mut POINT) -> BOOL;
    pub fn SetTimer(hwnd: HWND, id: usize, ms: u32, func: *const c_void) -> usize;
    pub fn LoadCursorW(inst: HINSTANCE, name: *const u16) -> HCURSOR;
    pub fn RegisterWindowMessageW(name: *const u16) -> u32;
    pub fn SetWindowTextW(hwnd: HWND, text: *const u16) -> BOOL;
    pub fn SendMessageW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT;
    pub fn PostMessageW(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> BOOL;
    pub fn MoveWindow(hwnd: HWND, x: i32, y: i32, w: i32, h: i32, repaint: BOOL) -> BOOL;
    pub fn SetWindowPos(
        hwnd: HWND,
        after: HWND,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> BOOL;
    pub fn GetClientRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    pub fn CreateIconIndirect(info: *const ICONINFO) -> HICON;
    pub fn DestroyIcon(icon: HICON) -> BOOL;
    pub fn GetSystemMetrics(index: i32) -> i32;
    pub fn OpenClipboard(hwnd: HWND) -> BOOL;
    pub fn EmptyClipboard() -> BOOL;
    pub fn SetClipboardData(format: u32, mem: HANDLE) -> HANDLE;
    pub fn CloseClipboard() -> BOOL;
    pub fn MessageBoxW(hwnd: HWND, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(msg: u32, data: *const NOTIFYICONDATAW) -> BOOL;
    pub fn ShellExecuteW(
        hwnd: HWND,
        op: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        show: i32,
    ) -> HINSTANCE;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
    pub fn GetProcAddress(module: HINSTANCE, name: *const u8) -> *const c_void;
    pub fn CreateMutexW(attrs: *const c_void, owner: BOOL, name: *const u16) -> HANDLE;
    pub fn GetLastError() -> u32;
    pub fn GetDiskFreeSpaceExW(
        dir: *const u16,
        free_to_caller: *mut u64,
        total: *mut u64,
        total_free: *mut u64,
    ) -> BOOL;
    pub fn GetVolumeInformationW(
        root: *const u16,
        vol_name: *mut u16,
        vol_name_len: u32,
        serial: *mut u32,
        max_component: *mut u32,
        flags: *mut u32,
        fs_name: *mut u16,
        fs_name_len: u32,
    ) -> BOOL;
    pub fn GlobalAlloc(flags: u32, bytes: usize) -> HANDLE;
    pub fn GlobalLock(mem: HANDLE) -> *mut c_void;
    pub fn GlobalUnlock(mem: HANDLE) -> BOOL;
    pub fn GlobalFree(mem: HANDLE) -> HANDLE;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetCurrentProcessId() -> u32;
    pub fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> BOOL;
    pub fn CloseHandle(h: HANDLE) -> BOOL;
    pub fn LocalFree(mem: HANDLE) -> HANDLE;
    pub fn GetLocalTime(st: *mut SYSTEMTIME);
    pub fn FormatMessageW(
        flags: u32,
        source: *const c_void,
        id: u32,
        lang: u32,
        buf: *mut u16,
        size: u32,
        args: *const c_void,
    ) -> u32;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn RegOpenKeyExW(root: HKEY, sub: *const u16, opts: u32, sam: u32, out: *mut HKEY) -> i32;
    pub fn RegQueryValueExW(
        key: HKEY,
        name: *const u16,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        len: *mut u32,
    ) -> i32;
    pub fn RegEnumValueW(
        key: HKEY,
        index: u32,
        name: *mut u16,
        name_len: *mut u32,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        data_len: *mut u32,
    ) -> i32;
    pub fn RegCloseKey(key: HKEY) -> i32;
    pub fn OpenProcessToken(process: HANDLE, access: u32, token: *mut HANDLE) -> BOOL;
    pub fn GetTokenInformation(
        token: HANDLE,
        class: u32,
        info: *mut c_void,
        len: u32,
        ret_len: *mut u32,
    ) -> BOOL;
    pub fn ConvertSidToStringSidW(sid: *mut c_void, out: *mut *mut u16) -> BOOL;
    pub fn OpenSCManagerW(machine: *const u16, db: *const u16, access: u32) -> SC_HANDLE;
    pub fn OpenServiceW(scm: SC_HANDLE, name: *const u16, access: u32) -> SC_HANDLE;
    pub fn QueryServiceStatus(svc: SC_HANDLE, status: *mut SERVICE_STATUS) -> BOOL;
    pub fn CloseServiceHandle(h: SC_HANDLE) -> BOOL;
}

#[link(name = "version")]
extern "system" {
    pub fn GetFileVersionInfoSizeW(file: *const u16, handle: *mut u32) -> u32;
    pub fn GetFileVersionInfoW(file: *const u16, handle: u32, len: u32, data: *mut c_void) -> BOOL;
    pub fn VerQueryValueW(
        block: *const c_void,
        sub: *const u16,
        buf: *mut *mut c_void,
        len: *mut u32,
    ) -> BOOL;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateDIBSection(
        hdc: HDC,
        bmi: *const BITMAPINFO,
        usage: u32,
        bits: *mut *mut c_void,
        section: HANDLE,
        offset: u32,
    ) -> HBITMAP;
    pub fn CreateBitmap(w: i32, h: i32, planes: u32, bpp: u32, bits: *const c_void) -> HBITMAP;
    pub fn DeleteObject(obj: HGDIOBJ) -> BOOL;
    pub fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strikeout: u32,
        charset: u32,
        out_precision: u32,
        clip_precision: u32,
        quality: u32,
        pitch_family: u32,
        face: *const u16,
    ) -> HFONT;
}


// ================================================================ v0.2: painted UI

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [u8; 32],
}

#[repr(C)]
pub struct DRAWITEMSTRUCT {
    pub CtlType: u32,
    pub CtlID: u32,
    pub itemID: u32,
    pub itemAction: u32,
    pub itemState: u32,
    pub hwndItem: HWND,
    pub hDC: HDC,
    pub rcItem: RECT,
    pub itemData: usize,
}

#[repr(C)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}

#[repr(C)]
pub struct INITCOMMONCONTROLSEX {
    pub dwSize: u32,
    pub dwICC: u32,
}

#[repr(C)]
pub struct LVCOLUMNW {
    pub mask: u32,
    pub fmt: i32,
    pub cx: i32,
    pub pszText: *mut u16,
    pub cchTextMax: i32,
    pub iSubItem: i32,
    pub iImage: i32,
    pub iOrder: i32,
    pub cxMin: i32,
    pub cxDefault: i32,
    pub cxIdeal: i32,
}

#[repr(C)]
pub struct LVITEMW {
    pub mask: u32,
    pub iItem: i32,
    pub iSubItem: i32,
    pub state: u32,
    pub stateMask: u32,
    pub pszText: *mut u16,
    pub cchTextMax: i32,
    pub iImage: i32,
    pub lParam: isize,
    pub iIndent: i32,
    pub iGroupId: i32,
    pub cColumns: u32,
    pub puColumns: *mut u32,
    pub piColFmt: *mut i32,
    pub iGroup: i32,
}

pub const WM_PAINT: u32 = 0x000F;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_SETTINGCHANGE: u32 = 0x001A;
pub const WM_SETCURSOR: u32 = 0x0020;
pub const WM_GETMINMAXINFO: u32 = 0x0024;
pub const WM_DRAWITEM: u32 = 0x002B;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_CTLCOLOREDIT: u32 = 0x0133;
pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
pub const WS_CLIPCHILDREN: u32 = 0x0200_0000;
pub const BS_OWNERDRAW: u32 = 0x000B;
pub const ODS_SELECTED: u32 = 0x0001;
pub const ODS_FOCUS: u32 = 0x0010;
pub const ODS_NOFOCUSRECT: u32 = 0x0200;
pub const VK_TAB: usize = 0x09;
pub const VK_SHIFT: i32 = 0x10;
pub const VK_CONTROL: i32 = 0x11;
pub const VK_PRIOR: usize = 0x21;
pub const VK_NEXT: usize = 0x22;
pub const HTCLIENT: isize = 1;
pub const IDC_HAND: usize = 32649;
pub const SRCCOPY: u32 = 0x00CC_0020;

pub const ICC_LISTVIEW_CLASSES: u32 = 0x0001;
pub const LVS_REPORT: u32 = 0x0001;
pub const LVS_SINGLESEL: u32 = 0x0004;
pub const LVS_SHOWSELALWAYS: u32 = 0x0008;
pub const LVS_NOSORTHEADER: u32 = 0x8000;
pub const LVS_EX_FULLROWSELECT: isize = 0x0020;
pub const LVS_EX_DOUBLEBUFFER: isize = 0x0001_0000;
pub const LVM_SETBKCOLOR: u32 = 0x1001;
pub const LVM_DELETEALLITEMS: u32 = 0x1009;
pub const LVM_SETCOLUMNWIDTH: u32 = 0x101E;
pub const LVM_SETTEXTCOLOR: u32 = 0x1024;
pub const LVM_SETTEXTBKCOLOR: u32 = 0x1026;
pub const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = 0x1036;
pub const LVM_INSERTITEMW: u32 = 0x104D;
pub const LVM_INSERTCOLUMNW: u32 = 0x1061;
pub const LVM_SETITEMTEXTW: u32 = 0x1074;
pub const LVCF_WIDTH: u32 = 0x0002;
pub const LVCF_TEXT: u32 = 0x0004;
pub const LVCF_SUBITEM: u32 = 0x0008;
pub const LVIF_TEXT: u32 = 0x0001;
pub const LVSCW_AUTOSIZE_USEHEADER: isize = -2;
pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

#[link(name = "user32")]
extern "system" {
    pub fn BeginPaint(hwnd: HWND, ps: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hwnd: HWND, ps: *const PAINTSTRUCT) -> BOOL;
    pub fn InvalidateRect(hwnd: HWND, rect: *const RECT, erase: BOOL) -> BOOL;
    pub fn GetKeyState(vk: i32) -> i16;
    pub fn ScreenToClient(hwnd: HWND, pt: *mut POINT) -> BOOL;
    pub fn SetCursor(cursor: HCURSOR) -> HCURSOR;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn CreateCompatibleBitmap(hdc: HDC, w: i32, h: i32) -> HBITMAP;
    pub fn SelectObject(hdc: HDC, obj: HGDIOBJ) -> HGDIOBJ;
    pub fn BitBlt(dst: HDC, x: i32, y: i32, w: i32, h: i32, src: HDC, sx: i32, sy: i32, rop: u32) -> BOOL;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn CreateSolidBrush(color: u32) -> HBRUSH;
    pub fn SetTextColor(hdc: HDC, color: u32) -> u32;
    pub fn SetBkColor(hdc: HDC, color: u32) -> u32;
}

#[link(name = "comctl32")]
extern "system" {
    pub fn InitCommonControlsEx(icc: *const INITCOMMONCONTROLSEX) -> BOOL;
}

#[link(name = "uxtheme")]
extern "system" {
    pub fn SetWindowTheme(hwnd: HWND, app: *const u16, idlist: *const u16) -> i32;
}

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmSetWindowAttribute(hwnd: HWND, attr: u32, value: *const c_void, size: u32) -> i32;
}

// ---- tooltips (v0.2.1)
#[repr(C)]
pub struct TTTOOLINFOW {
    pub cbSize: u32,
    pub uFlags: u32,
    pub hwnd: HWND,
    pub uId: usize,
    pub rect: RECT,
    pub hinst: HINSTANCE,
    pub lpszText: *mut u16,
    pub lParam: LPARAM,
    pub lpReserved: *mut c_void,
}
/// Size without lpReserved: accepted by both comctl32 v5 and v6.
pub const TTTOOLINFOW_V2_SIZE: u32 = 64;
pub const ICC_TAB_CLASSES: u32 = 0x0008; // includes tooltips
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub const TTS_ALWAYSTIP: u32 = 0x01;
pub const TTS_NOPREFIX: u32 = 0x02;
pub const TTF_SUBCLASS: u32 = 0x0010;
pub const TTM_SETDELAYTIME: u32 = 0x0403;
pub const TTM_SETMAXTIPWIDTH: u32 = 0x0418;
pub const TTM_ADDTOOLW: u32 = 0x0432;
pub const TTM_DELTOOLW: u32 = 0x0433;
pub const TTDT_AUTOPOP: usize = 2;

// Compile-time layout checks against the Windows x64 ABI sizes.
#[cfg(target_pointer_width = "64")]
const _: () = {
    use std::mem::size_of;
    assert!(size_of::<NOTIFYICONDATAW>() == 976);
    assert!(size_of::<MSG>() == 48);
    assert!(size_of::<WNDCLASSEXW>() == 80);
    assert!(size_of::<BITMAPINFOHEADER>() == 40);
    assert!(size_of::<ICONINFO>() == 32);
    assert!(size_of::<SERVICE_STATUS>() == 28);
    assert!(size_of::<VS_FIXEDFILEINFO>() == 52);
    assert!(size_of::<SYSTEMTIME>() == 16);
    assert!(size_of::<PAINTSTRUCT>() == 72);
    assert!(size_of::<DRAWITEMSTRUCT>() == 64);
    assert!(size_of::<MINMAXINFO>() == 40);
    assert!(size_of::<LVCOLUMNW>() == 56);
    assert!(size_of::<LVITEMW>() == 88);
    assert!(size_of::<TTTOOLINFOW>() == 72);
    assert!(HKEY_LOCAL_MACHINE as usize == 0xFFFF_FFFF_8000_0002);
};
