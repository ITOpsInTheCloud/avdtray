//! Small safe wrappers around the raw Win32 calls in `ffi`.

use crate::ffi::*;
use core::ffi::c_void;
use std::ptr::{null, null_mut};

/// UTF-16, NUL-terminated.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Copy `s` into a fixed-size UTF-16 buffer, truncating and NUL-terminating.
pub fn copy_fixed(dst: &mut [u16], s: &str) {
    let max = dst.len().saturating_sub(1);
    let mut n = 0;
    for (i, c) in s.encode_utf16().take(max).enumerate() {
        dst[i] = c;
        n = i + 1;
    }
    if n < dst.len() {
        dst[n] = 0;
    }
}

fn bytes_to_u16(data: &[u8]) -> Vec<u16> {
    data.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

// ------------------------------------------------------------------ registry

pub struct RegKey(HKEY);

impl RegKey {
    pub fn open(root: HKEY, path: &str) -> Option<RegKey> {
        let p = wide(path);
        let mut h: HKEY = 0;
        let rc = unsafe { RegOpenKeyExW(root, p.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut h) };
        if rc == ERROR_SUCCESS {
            Some(RegKey(h))
        } else {
            None
        }
    }

    fn raw(&self, name: &str) -> Option<(u32, Vec<u8>)> {
        let n = wide(name);
        let mut kind = 0u32;
        let mut len = 0u32;
        let rc = unsafe { RegQueryValueExW(self.0, n.as_ptr(), null_mut(), &mut kind, null_mut(), &mut len) };
        if rc != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u8; len as usize + 2];
        for _ in 0..3 {
            let mut cb = buf.len() as u32;
            let rc = unsafe {
                RegQueryValueExW(self.0, n.as_ptr(), null_mut(), &mut kind, buf.as_mut_ptr(), &mut cb)
            };
            if rc == ERROR_SUCCESS {
                buf.truncate(cb as usize);
                return Some((kind, buf));
            }
            if rc != ERROR_MORE_DATA {
                return None;
            }
            buf.resize(cb as usize + 2, 0);
        }
        None
    }

    /// Read a number. Tolerates REG_SZ values containing a decimal or 0x-hex number.
    pub fn dword(&self, name: &str) -> Option<u32> {
        let (kind, data) = self.raw(name)?;
        match kind {
            REG_DWORD if data.len() >= 4 => Some(u32::from_le_bytes([data[0], data[1], data[2], data[3]])),
            REG_QWORD if data.len() >= 8 => Some(u32::from_le_bytes([data[0], data[1], data[2], data[3]])),
            REG_SZ | REG_EXPAND_SZ => {
                let s = from_wide(&bytes_to_u16(&data));
                let s = s.trim();
                if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                    u32::from_str_radix(h, 16).ok()
                } else {
                    s.parse().ok()
                }
            }
            _ => None,
        }
    }

    pub fn string(&self, name: &str) -> Option<String> {
        let (kind, data) = self.raw(name)?;
        Some(format_value(kind, &data))
    }

    /// All values of the key as (name, formatted value).
    pub fn values(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut name = vec![0u16; 16384];
        let mut data = vec![0u8; 64 * 1024];
        let mut index = 0u32;
        loop {
            let mut name_len = name.len() as u32;
            let mut data_len = data.len() as u32;
            let mut kind = 0u32;
            let rc = unsafe {
                RegEnumValueW(
                    self.0,
                    index,
                    name.as_mut_ptr(),
                    &mut name_len,
                    null_mut(),
                    &mut kind,
                    data.as_mut_ptr(),
                    &mut data_len,
                )
            };
            if rc == ERROR_NO_MORE_ITEMS {
                break;
            }
            if rc == ERROR_SUCCESS {
                let n = String::from_utf16_lossy(&name[..name_len as usize]);
                let n = if n.is_empty() { "(Default)".to_string() } else { n };
                out.push((n, format_value(kind, &data[..data_len as usize])));
            } else if rc != ERROR_MORE_DATA {
                break;
            }
            index += 1;
            if index > 4096 {
                break;
            }
        }
        out.sort_by_key(|a| a.0.to_lowercase());
        out
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn format_value(kind: u32, data: &[u8]) -> String {
    match kind {
        REG_SZ | REG_EXPAND_SZ => from_wide(&bytes_to_u16(data)),
        REG_MULTI_SZ => {
            let w = bytes_to_u16(data);
            w.split(|&c| c == 0)
                .filter(|s| !s.is_empty())
                .map(String::from_utf16_lossy)
                .collect::<Vec<_>>()
                .join("; ")
        }
        REG_DWORD if data.len() >= 4 => {
            let v = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            format!("{v} (0x{v:X})")
        }
        REG_QWORD if data.len() >= 8 => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&data[..8]);
            let v = u64::from_le_bytes(b);
            format!("{v} (0x{v:X})")
        }
        REG_BINARY => {
            let hex: Vec<String> = data.iter().take(32).map(|b| format!("{b:02X}")).collect();
            let more = if data.len() > 32 { " …" } else { "" };
            format!("{}{}", hex.join(" "), more)
        }
        other => format!("<type {other}, {} bytes>", data.len()),
    }
}

// ------------------------------------------------------------------ identity

pub fn current_user_sid() -> Option<String> {
    unsafe {
        let mut token: HANDLE = 0;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut len = 0u32;
        GetTokenInformation(token, TOKEN_USER_CLASS, null_mut(), 0, &mut len);
        let mut result = None;
        if len > 0 {
            // u64 buffer so the embedded SID pointer is properly aligned.
            let mut buf = vec![0u64; (len as usize).div_ceil(8)];
            if GetTokenInformation(token, TOKEN_USER_CLASS, buf.as_mut_ptr() as *mut c_void, len, &mut len) != 0 {
                let sid = *(buf.as_ptr() as *const *mut c_void);
                let mut s: *mut u16 = null_mut();
                if ConvertSidToStringSidW(sid, &mut s) != 0 && !s.is_null() {
                    let mut n = 0;
                    while *s.add(n) != 0 {
                        n += 1;
                    }
                    result = Some(String::from_utf16_lossy(std::slice::from_raw_parts(s, n)));
                    LocalFree(s as HANDLE);
                }
            }
        }
        CloseHandle(token);
        result
    }
}

pub fn session_id() -> Option<u32> {
    let mut id = 0u32;
    let ok = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut id) };
    if ok != 0 {
        Some(id)
    } else {
        None
    }
}

pub fn now_string() -> String {
    let mut st = SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut st) };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    )
}

// ------------------------------------------------------------------ services

/// Ok(state) or Err(win32 error). 1060 = service not installed.
pub fn service_state(name: &str) -> Result<u32, u32> {
    unsafe {
        let scm = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
        if scm == 0 {
            return Err(GetLastError());
        }
        let n = wide(name);
        let svc = OpenServiceW(scm, n.as_ptr(), SERVICE_QUERY_STATUS);
        let res = if svc == 0 {
            Err(GetLastError())
        } else {
            let mut st = SERVICE_STATUS::default();
            let r = if QueryServiceStatus(svc, &mut st) != 0 {
                Ok(st.dwCurrentState)
            } else {
                Err(GetLastError())
            };
            CloseServiceHandle(svc);
            r
        };
        CloseServiceHandle(scm);
        res
    }
}

pub fn service_state_text(s: &Result<u32, u32>) -> &'static str {
    match s {
        Ok(1) => "Stopped",
        Ok(2) => "Starting",
        Ok(3) => "Stopping",
        Ok(4) => "Running",
        Ok(5) => "Resuming",
        Ok(6) => "Pausing",
        Ok(7) => "Paused",
        Ok(_) => "Unknown",
        Err(1060) => "Not installed",
        Err(5) => "Access denied",
        Err(_) => "Unavailable",
    }
}

// ------------------------------------------------------------------ files

pub fn file_version(path: &str) -> Option<String> {
    unsafe {
        let p = wide(path);
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(p.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        if GetFileVersionInfoW(p.as_ptr(), 0, size, buf.as_mut_ptr() as *mut c_void) == 0 {
            return None;
        }
        let root = wide("\\");
        let mut ptr: *mut c_void = null_mut();
        let mut len = 0u32;
        if VerQueryValueW(buf.as_ptr() as *const c_void, root.as_ptr(), &mut ptr, &mut len) == 0
            || ptr.is_null()
            || (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
        {
            return None;
        }
        let fi: VS_FIXEDFILEINFO = std::ptr::read_unaligned(ptr as *const VS_FIXEDFILEINFO);
        Some(format!(
            "{}.{}.{}.{}",
            fi.dwFileVersionMS >> 16,
            fi.dwFileVersionMS & 0xFFFF,
            fi.dwFileVersionLS >> 16,
            fi.dwFileVersionLS & 0xFFFF
        ))
    }
}

pub struct VolumeInfo {
    pub total: u64,
    pub free: u64,
    pub read_only: bool,
    pub fs: String,
    pub label: String,
}

/// Works with drive letters, mount folders and `\\?\Volume{GUID}\` paths.
pub fn volume_info(path: &str) -> Option<VolumeInfo> {
    let mut root = path.trim().to_string();
    if root.is_empty() {
        return None;
    }
    if !root.ends_with('\\') {
        root.push('\\');
    }
    let w = wide(&root);
    unsafe {
        let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
        if GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) == 0 {
            return None;
        }
        let mut label = [0u16; 261];
        let mut fs = [0u16; 261];
        let (mut serial, mut maxc, mut flags) = (0u32, 0u32, 0u32);
        let ok = GetVolumeInformationW(
            w.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            &mut serial,
            &mut maxc,
            &mut flags,
            fs.as_mut_ptr(),
            fs.len() as u32,
        ) != 0;
        Some(VolumeInfo {
            total,
            free,
            read_only: ok && flags & FILE_READ_ONLY_VOLUME != 0,
            fs: if ok { from_wide(&fs) } else { String::new() },
            label: if ok { from_wide(&label) } else { String::new() },
        })
    }
}

pub fn error_text(code: u32) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            null(),
            code,
            0,
            buf.as_mut_ptr(),
            buf.len() as u32,
            null(),
        )
    };
    String::from_utf16_lossy(&buf[..n as usize]).trim().to_string()
}

// ------------------------------------------------------------------ shell / UI helpers

pub fn set_clipboard(hwnd: HWND, text: &str) -> bool {
    let w = wide(text);
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            return false;
        }
        EmptyClipboard();
        let mem = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2);
        let mut ok = false;
        if mem != 0 {
            let p = GlobalLock(mem) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(w.as_ptr(), p, w.len());
                GlobalUnlock(mem);
                ok = SetClipboardData(CF_UNICODETEXT, mem) != 0;
            }
            if !ok {
                GlobalFree(mem);
            }
        }
        CloseClipboard();
        ok
    }
}

pub fn shell_open(file: &str, params: Option<&str>) -> bool {
    let op = wide("open");
    let f = wide(file);
    let p = params.map(wide);
    let r = unsafe {
        ShellExecuteW(
            0,
            op.as_ptr(),
            f.as_ptr(),
            p.as_ref().map_or(null(), |v| v.as_ptr()),
            null(),
            SW_SHOWNORMAL,
        )
    };
    r > 32
}

pub fn message_box(hwnd: HWND, text: &str, caption: &str, flags: u32) {
    let t = wide(text);
    let c = wide(caption);
    unsafe {
        MessageBoxW(hwnd, t.as_ptr(), c.as_ptr(), flags);
    }
}

/// GetDpiForWindow is Windows 10 1607+; resolve dynamically and fall back to 96.
pub fn dpi_for_window(hwnd: HWND) -> u32 {
    type F = unsafe extern "system" fn(HWND) -> u32;
    unsafe {
        let user32 = wide("user32.dll");
        let m = GetModuleHandleW(user32.as_ptr());
        if m == 0 {
            return 96;
        }
        let p = GetProcAddress(m, c"GetDpiForWindow".as_ptr() as *const u8);
        if p.is_null() {
            return 96;
        }
        let f: F = std::mem::transmute::<*const c_void, F>(p);
        match f(hwnd) {
            0 => 96,
            d => d,
        }
    }
}

pub fn fmt_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}
