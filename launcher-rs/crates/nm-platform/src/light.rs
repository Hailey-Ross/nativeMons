//! Light mode's automatic choice (crossplatform note section 8): few cores or little memory, or a handheld or
//! Wine/Proton running on battery. Thresholds until the C# baseline measurement gives better ones.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Machine {
    pub cores: usize,
    pub ram_gb: f32,
    pub on_battery: bool,
    pub steam_deck: bool,
    pub wine: bool,
}

pub const FEW_CORES: usize = 4;
pub const LITTLE_RAM_GB: f32 = 6.0;

/// (Light mode on, why) for the Settings row's "Auto turns it on/off on this PC: <why>".
pub fn decide(m: &Machine) -> (bool, &'static str) {
    if m.cores > 0 && m.cores <= FEW_CORES { return (true, "it has 4 or fewer processor threads"); }
    if m.ram_gb > 0.0 && m.ram_gb < LITTLE_RAM_GB { return (true, "it has less than 6 GB of memory"); }
    if m.on_battery && m.steam_deck { return (true, "a Steam Deck on battery"); }
    if m.on_battery && m.wine { return (true, "Wine or Proton on battery"); }
    (false, "it has the processor and memory for full rate")
}

pub fn machine() -> Machine {
    Machine {
        cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        ram_gb: ram_gb(),
        on_battery: on_battery(),
        steam_deck: std::env::var("SteamDeck").as_deref() == Ok("1"),
        wine: wine(),
    }
}

pub fn detect() -> (bool, &'static str) { decide(&machine()) }

#[cfg(windows)]
fn ram_gb() -> f32 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut s: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    s.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut s) } == 0 { return 0.0; }
    s.ullTotalPhys as f32 / 1073741824.0
}
#[cfg(windows)]
fn on_battery() -> bool {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // ACLineStatus 0: offline (battery); 128 = no battery at all.
    unsafe { GetSystemPowerStatus(&mut s) != 0 && s.ACLineStatus == 0 }
}
/// Wine's ntdll exports wine_get_version.
#[cfg(windows)]
fn wine() -> bool {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    unsafe {
        let h = GetModuleHandleA(b"ntdll.dll\0".as_ptr());
        h != 0 && GetProcAddress(h, b"wine_get_version\0".as_ptr()).is_some()
    }
}

#[cfg(unix)]
fn ram_gb() -> f32 {
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|t| parse_meminfo(&t)).unwrap_or(0.0)
}
#[cfg(unix)]
fn on_battery() -> bool {
    let Ok(rd) = std::fs::read_dir("/sys/class/power_supply") else { return false };
    let mut has_battery = false;
    for e in rd.flatten() {
        let p = e.path();
        let kind = std::fs::read_to_string(p.join("type")).unwrap_or_default();
        if kind.trim() == "Mains" && std::fs::read_to_string(p.join("online")).unwrap_or_default().trim() == "1" { return false; }
        if kind.trim() == "Battery" { has_battery = true; }
    }
    has_battery
}
#[cfg(unix)]
fn wine() -> bool { false }

/// MemTotal in GB from /proc/meminfo's text.
pub fn parse_meminfo(t: &str) -> Option<f32> {
    let line = t.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: f32 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1048576.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thresholds() {
        let big = Machine { cores: 16, ram_gb: 32.0, ..Default::default() };
        assert!(!decide(&big).0);
        assert!(decide(&Machine { cores: 4, ..big }).0);
        assert!(!decide(&Machine { cores: 5, ..big }).0);
        assert!(decide(&Machine { ram_gb: 5.9, ..big }).0);
        assert!(!decide(&Machine { ram_gb: 6.0, ..big }).0);
        assert!(decide(&Machine { on_battery: true, steam_deck: true, ..big }).0);
        assert!(decide(&Machine { on_battery: true, wine: true, ..big }).0);
        assert!(!decide(&Machine { on_battery: true, ..big }).0);
        assert!(!decide(&Machine { steam_deck: true, ..big }).0);
        // Unknown counts (0) never turn it on.
        assert!(!decide(&Machine { cores: 0, ram_gb: 0.0, ..Default::default() }).0);
    }
    #[test]
    fn meminfo() {
        assert_eq!(parse_meminfo("MemTotal:       16303852 kB\nMemFree: 1 kB"), Some(16303852.0 / 1048576.0));
        assert_eq!(parse_meminfo("nothing"), None);
    }
    #[test]
    fn this_machine() {
        let m = machine();
        assert!(m.cores > 0 && m.ram_gb > 0.5);
        assert!(!m.wine);
    }
}
