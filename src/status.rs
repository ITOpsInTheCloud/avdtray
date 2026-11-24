//! Collects FSLogix state for the signed-in user and renders it as text.
//!
//! Data sources (all cheap, no log parsing on the timer):
//!   * HKLM\SOFTWARE\FSLogix\Profiles[\Sessions\<SID>]
//!   * HKLM\SOFTWARE\Policies\FSLogix\ODFC[\Sessions\<SID>]  (+ HKCU\Software\FSLogix\ODFC\Sessions)
//!   * Service Control Manager for frxsvc / frxccds / drivers
//!   * File versions of the FSLogix binaries (read once)

use crate::ffi::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use crate::win::{self, RegKey, VolumeInfo};
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Light {
    Grey = 0,
    Green = 1,
    Yellow = 2,
    Red = 3,
}

impl Light {
    pub fn word(self) -> &'static str {
        match self {
            Light::Grey => "INACTIVE",
            Light::Green => "OK",
            Light::Yellow => "WARNING",
            Light::Red => "ERROR",
        }
    }
}

pub struct Container {
    pub name: &'static str,
    pub config_path: String,
    pub enabled: Option<u32>,
    pub session_path: Option<String>,
    pub status: Option<u32>,
    pub reason: Option<u32>,
    pub error: Option<u32>,
    pub values: Vec<(String, String)>,
    pub volume: Option<VolumeInfo>,
    pub light: Light,
    pub summary: String,
    /// Enabled on this host or has session data for this user.
    pub relevant: bool,
}

pub struct Component {
    pub name: &'static str,
    pub display: &'static str,
    pub state: Result<u32, u32>,
    pub version: Option<String>,
}

pub struct Snapshot {
    pub user: String,
    pub computer: String,
    pub sid: Option<String>,
    pub session: Option<u32>,
    pub profile: Container,
    pub odfc: Container,
    pub components: Vec<Component>,
    pub light: Light,
    pub headline: String,
}

// ------------------------------------------------------------------ code tables
// Source: https://learn.microsoft.com/fslogix/troubleshooting-error-codes

pub fn status_text(code: u32) -> &'static str {
    match code {
        0 => "Success",
        1 => "Can't load user's profile",
        2 => "Virtual disk API isn't available on this platform",
        3 => "Can't retrieve user's security identifier",
        4 => "There was an error setting up the Office 365 Container",
        5 => "Can't retrieve security information",
        6 => "Can't retrieve virtual disk location",
        7 => "Can't create destination folders",
        8 => "Can't impersonate user",
        9 => "Can't create virtual disk",
        10 => "Can't release virtual disk",
        11 => "Can't open virtual disk",
        12 => "Can't attach to virtual disk",
        13 => "Can't retrieve virtual disk's physical information",
        14 => "Can't open virtual disk's volume",
        15 => "Can't initialize virtual disk",
        16 => "Can't retrieve virtual disk identifier",
        17 => "Error while formatting virtual disk",
        18 => "Can't retrieve profile directory",
        19 => "Can't set up directory mount point",
        20 => "Can't import registry information",
        21 => "Can't retrieve user's group",
        22 => "Error handling profile",
        23 => "Can't set up folder redirections",
        24 => "Unable to create event",
        25 => "Maximum sessions reached",
        26 => "Can't detach virtual disk at the provided location",
        27 => "Can't find virtual disk at the provided location",
        28 => "No user session config found",
        100 => "Waiting for the Windows Profile Service to determine the user's profile folder",
        200 => "Setup in progress",
        300 => "The profile is already attached (differencing disks only)",
        _ => "Unknown status code",
    }
}

pub fn reason_text(code: u32) -> &'static str {
    match code {
        0 => "The container is attached",
        1 => "User isn't a member of the include group",
        2 => "User is a member of the exclude group",
        3 => "A local profile for this user exists on this system",
        4 => "Not an appropriate user type",
        5 => "Reason initialized to empty state",
        6 => "Component isn't enabled in product key (legacy)",
        7 => "Profile is a Windows temporary profile",
        8 => "Session isn't an Azure Virtual Desktop session",
        9 => "Profile load failed",
        _ => "Unknown reason code",
    }
}

// ------------------------------------------------------------------ paths

pub fn apps_dir() -> PathBuf {
    if let Some(p) = RegKey::open(HKEY_LOCAL_MACHINE, r"SOFTWARE\FSLogix\Apps").and_then(|k| k.string("InstallPath")) {
        if !p.trim().is_empty() {
            return PathBuf::from(p.trim());
        }
    }
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    PathBuf::from(pf).join("FSLogix").join("Apps")
}

pub fn logs_dir() -> PathBuf {
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    PathBuf::from(pd).join("FSLogix").join("Logs")
}

/// Newest *.log file in %ProgramData%\FSLogix\Logs\<sub>.
pub fn newest_log(sub: &str) -> Option<PathBuf> {
    std::fs::read_dir(logs_dir().join(sub))
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x.eq_ignore_ascii_case("log"))
                .unwrap_or(false)
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

// ------------------------------------------------------------------ collection

fn evaluate(
    name: &'static str,
    config_paths: &[&str],
    sessions: &[(HKEY, &str, String)],
) -> Container {
    // `Enabled` can live in the plain or the Policies branch depending on how it was deployed
    // (GPO/ADMX vs. script/Intune). The first location that has the value wins.
    let (config_path, enabled) = config_paths
        .iter()
        .find_map(|p| {
            RegKey::open(HKEY_LOCAL_MACHINE, p)
                .and_then(|k| k.dword("Enabled"))
                .map(|v| (p.to_string(), Some(v)))
        })
        .unwrap_or_else(|| (config_paths.join(" or HKLM\\"), None));

    let mut c = Container {
        name,
        config_path,
        enabled,
        session_path: None,
        status: None,
        reason: None,
        error: None,
        values: Vec::new(),
        volume: None,
        light: Light::Grey,
        summary: String::new(),
        relevant: false,
    };

    for (root, root_name, path) in sessions {
        if let Some(k) = RegKey::open(*root, path) {
            let status = k.dword("Status");
            // The HKCU fallback location only counts if it really holds session state.
            if status.is_none() && *root == HKEY_CURRENT_USER {
                continue;
            }
            c.session_path = Some(format!(r"{root_name}\{path}"));
            c.status = status;
            c.reason = k.dword("Reason");
            c.error = k.dword("Error");
            c.values = k.values();
            break;
        }
    }

    // FSLogix records the container's volume (usually \\?\Volume{GUID}\) in the session key.
    if let Some((_, vol)) = c.values.iter().find(|(n, _)| n.eq_ignore_ascii_case("Volume")) {
        c.volume = win::volume_info(vol);
    }

    let is_enabled = enabled == Some(1);
    let attached = matches!(c.status, Some(0) | Some(300)) && matches!(c.reason, None | Some(0));
    // FSLogix also writes session keys for containers that are switched off (e.g. ODFC with
    // Status 1 / Reason 6 when Office data lives in the profile container). Those must not
    // count. A container that is genuinely attached always counts, wherever Enabled lives.
    c.relevant = is_enabled || attached;

    let (light, summary) = match (&c.session_path, c.status) {
        (None, _) if !is_enabled => (Light::Grey, "Not enabled on this host".to_string()),
        (Some(_), _) if !c.relevant => (
            Light::Grey,
            "Not enabled on this host (session values below are informational only)".to_string(),
        ),
        (None, _) => (
            Light::Yellow,
            "Enabled, but no session data for this user yet".to_string(),
        ),
        (Some(_), None) => (Light::Yellow, "Session key has no Status value".to_string()),
        (Some(_), Some(0)) | (Some(_), Some(300)) => match c.reason {
            None | Some(0) => {
                let mode = match &c.volume {
                    Some(v) if v.read_only => " (read-only)",
                    Some(_) => " (read/write)",
                    None => "",
                };
                (Light::Green, format!("Attached{mode}"))
            }
            Some(r) => (Light::Yellow, format!("Not attached: {}", reason_text(r))),
        },
        (Some(_), Some(s @ (100 | 200))) => (Light::Yellow, status_text(s).to_string()),
        (Some(_), Some(s)) => (Light::Red, format!("Error {s}: {}", status_text(s))),
    };
    c.light = light;
    c.summary = summary;
    c
}

const COMPONENTS: [(&str, &str, bool, &str); 5] = [
    ("frxsvc", "FSLogix Apps Services", true, "frxsvc.exe"),
    ("frxccds", "FSLogix Cloud Caching Service", true, "frxccds.exe"),
    ("frxdrv", "FSLogix kernel driver", false, "frxdrv.sys"),
    ("frxdrvvt", "FSLogix kernel virtualization driver", false, "frxdrvvt.sys"),
    ("frxccd", "FSLogix Cloud Cache driver", false, "frxccd.sys"),
];

/// Binary versions only change with an install (which needs a restart), so read them once.
fn versions() -> &'static Vec<Option<String>> {
    static V: OnceLock<Vec<Option<String>>> = OnceLock::new();
    V.get_or_init(|| {
        let apps = apps_dir();
        let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let drivers = PathBuf::from(windir).join("System32").join("drivers");
        COMPONENTS
            .iter()
            .map(|(_, _, in_apps, file)| {
                let p = if *in_apps { apps.join(file) } else { drivers.join(file) };
                win::file_version(&p.to_string_lossy())
            })
            .collect()
    })
}

pub fn collect(sid: Option<&str>) -> Snapshot {
    let sid_s = sid.unwrap_or("<unknown-sid>");

    let profile = evaluate(
        "Profile container",
        &[r"SOFTWARE\FSLogix\Profiles", r"SOFTWARE\Policies\FSLogix\Profiles"],
        &[(HKEY_LOCAL_MACHINE, "HKLM", format!(r"SOFTWARE\FSLogix\Profiles\Sessions\{sid_s}"))],
    );
    let odfc = evaluate(
        "Office container (ODFC)",
        &[r"SOFTWARE\Policies\FSLogix\ODFC", r"SOFTWARE\FSLogix\ODFC"],
        &[
            (HKEY_LOCAL_MACHINE, "HKLM", format!(r"SOFTWARE\Policies\FSLogix\ODFC\Sessions\{sid_s}")),
            (HKEY_CURRENT_USER, "HKCU", r"Software\FSLogix\ODFC\Sessions".to_string()),
        ],
    );

    let vers = versions();
    let components: Vec<Component> = COMPONENTS
        .iter()
        .zip(vers.iter())
        .map(|((name, display, _, _), v)| Component {
            name,
            display,
            state: win::service_state(name),
            version: v.clone(),
        })
        .collect();

    let svc = &components[0].state;
    let containers: Vec<&Container> = [&profile, &odfc].into_iter().filter(|c| c.relevant).collect();

    let (light, headline) = if containers.is_empty() {
        if matches!(svc, Err(1060)) {
            (Light::Grey, "FSLogix is not installed".to_string())
        } else {
            (Light::Grey, "FSLogix is not enabled on this host".to_string())
        }
    } else if !matches!(svc, Ok(4)) {
        (
            Light::Red,
            format!("FSLogix service (frxsvc) is {}", win::service_state_text(svc).to_lowercase()),
        )
    } else {
        let worst = containers.iter().map(|c| c.light).max().unwrap_or(Light::Grey);
        let text = containers
            .iter()
            .map(|c| format!("{}: {}", short(c.name), c.summary))
            .collect::<Vec<_>>()
            .join(" | ");
        (worst, text)
    };

    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(d), Ok(u)) => format!(r"{d}\{u}"),
        (_, Ok(u)) => u,
        _ => "<unknown>".into(),
    };

    Snapshot {
        user,
        computer: std::env::var("COMPUTERNAME").unwrap_or_default(),
        sid: sid.map(str::to_string),
        session: win::session_id(),
        profile,
        odfc,
        components,
        light,
        headline,
    }
}

fn short(name: &str) -> &str {
    if name.starts_with("Profile") {
        "Profile"
    } else {
        "Office"
    }
}

// ------------------------------------------------------------------ rendering

/// Tray tooltip, max 127 UTF-16 units (the API limit).
pub fn tooltip(s: &Snapshot) -> String {
    let mut t = String::from("AVD Tray");
    let containers: Vec<&Container> = [&s.profile, &s.odfc].into_iter().filter(|c| c.relevant).collect();
    if containers.is_empty() || s.light == Light::Red && !matches!(s.components[0].state, Ok(4)) {
        t.push('\n');
        t.push_str(&s.headline);
    } else {
        for c in containers {
            t.push('\n');
            t.push_str(&format!("{}: {}", short(c.name), c.summary));
        }
    }
    let units: Vec<u16> = t.encode_utf16().collect();
    if units.len() > 127 {
        let mut cut = String::from_utf16_lossy(&units[..124]);
        cut.push('…');
        cut
    } else {
        t
    }
}

fn container_section(out: &mut Vec<String>, c: &Container) {
    out.push(format!("=== {} ===", c.name));
    out.push(format!("State        : [{}] {}", c.light.word(), c.summary));
    out.push(format!(
        "Enabled      : {}   (HKLM\\{}\\Enabled)",
        c.enabled.map_or("not set".to_string(), |v| v.to_string()),
        c.config_path
    ));
    match &c.session_path {
        None => out.push("Session key  : not found".into()),
        Some(p) => {
            out.push(format!("Session key  : {p}"));
            if let Some(s) = c.status {
                out.push(format!("Status       : {s} - {}", status_text(s)));
            }
            if let Some(r) = c.reason {
                out.push(format!("Reason       : {r} - {}", reason_text(r)));
            }
            if let Some(e) = c.error {
                let txt = win::error_text(e);
                out.push(format!("Error        : {e} (0x{e:08X}){}", if txt.is_empty() { String::new() } else { format!(" - {txt}") }));
            }
        }
    }
    if let Some(v) = &c.volume {
        let pct = if v.total > 0 { v.free as f64 * 100.0 / v.total as f64 } else { 0.0 };
        out.push(format!(
            "Volume       : {} total, {} free ({pct:.0}%), {}{}, {}",
            win::fmt_bytes(v.total),
            win::fmt_bytes(v.free),
            if v.fs.is_empty() { "?" } else { &v.fs },
            if v.label.is_empty() { String::new() } else { format!(" \"{}\"", v.label) },
            if v.read_only { "READ-ONLY" } else { "read/write" }
        ));
    }
    if !c.values.is_empty() {
        out.push("Session values:".into());
        let w = c.values.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0).min(32);
        for (n, v) in &c.values {
            out.push(format!("  {n:<w$} = {v}"));
        }
    }
    out.push(String::new());
}

/// Full report body (no timestamp, so callers can detect real changes).
pub fn report_body(s: &Snapshot) -> String {
    let mut out: Vec<String> = Vec::new();
    out.push(format!("Computer     : {}", s.computer));
    out.push(format!("User         : {}", s.user));
    out.push(format!("SID          : {}", s.sid.as_deref().unwrap_or("<unknown>")));
    if let Some(id) = s.session {
        out.push(format!("Session ID   : {id}"));
    }
    out.push(String::new());
    out.push(format!("OVERALL      : [{}] {}", s.light.word(), s.headline));
    out.push(String::new());

    container_section(&mut out, &s.profile);
    container_section(&mut out, &s.odfc);

    out.push("=== FSLogix components ===".into());
    for c in &s.components {
        out.push(format!(
            "{:<9} {:<14} {:<16} {}",
            c.name,
            win::service_state_text(&c.state),
            c.version.as_deref().unwrap_or("-"),
            c.display
        ));
    }
    out.push(format!("Install path : {}", apps_dir().display()));
    out.push(String::new());

    out.push("=== Logs ===".into());
    for sub in ["Profile", "ODFC"] {
        out.push(format!(
            "{:<12} : {}",
            format!("{sub} log"),
            newest_log(sub).map_or("none found".to_string(), |p| p.display().to_string())
        ));
    }
    out.push("Event log    : Microsoft-FSLogix-Apps/Operational (event 25 = profile load)".into());

    out.join("\r\n")
}

pub fn report(body: &str, extra: &str) -> String {
    let mut r = format!(
        "AVD Tray {} - FSLogix status report\r\nGenerated    : {}\r\n\r\n{}",
        env!("CARGO_PKG_VERSION"),
        win::now_string(),
        body
    );
    if !extra.is_empty() {
        r.push_str("\r\n\r\n");
        r.push_str(extra);
    }
    r.push_str("\r\n");
    r
}

