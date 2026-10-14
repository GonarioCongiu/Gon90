//! Lettura dello stato del sistema e azioni rapide.
//!
//! Tutto passa da `/proc`, `/sys` e da strumenti standard di Arch/CachyOS
//! (`wpctl`, `nmcli`, `bluetoothctl`, `playerctl`, `brightnessctl`).
//! Se uno strumento manca, il relativo dato resta semplicemente vuoto.

use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    pub full: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Volume {
    pub level: f64,
    pub muted: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wifi {
    pub enabled: bool,
    pub ssid: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Music {
    pub playing: bool,
    pub player: String,
    pub artist: String,
    pub title: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Disk {
    pub used: f64,
    pub free_gb: f64,
    pub total_gb: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Weather {
    pub temp: String,
    pub condition: String,
    pub place: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SysSnapshot {
    pub battery: Option<Battery>,
    pub cpu: f64,
    pub cpu_hist: Vec<f64>,
    pub mem: f64,
    pub mem_hist: Vec<f64>,
    pub volume: Option<Volume>,
    pub brightness: Option<f64>,
    pub wifi: Option<Wifi>,
    pub bluetooth: Option<bool>,
    pub music: Option<Music>,
    pub disk: Option<Disk>,
    pub uptime: u64,
    pub load: f64,
    pub weather: Option<Weather>,
}

const HISTORY: usize = 40;

// ───────────────────────────── Utilità ─────────────────────────────

pub fn in_path(prog: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|dir| dir.join(prog).is_file()))
        .unwrap_or(false)
}

fn output(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Avvia un comando di shell staccato da Folio (sopravvive a un suo riavvio).
pub fn spawn(cmd: &str) {
    if cmd.trim().is_empty() {
        return;
    }
    let mut c = Command::new("sh");
    c.arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    match c.spawn() {
        Ok(mut child) => {
            // Raccoglie il processo terminato per non lasciare zombie.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => log::warn!("avvio di `{cmd}` fallito: {e}"),
    }
}

fn run(prog: &str, args: &[&str]) {
    let mut c = Command::new(prog);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Ok(mut child) = c.spawn() {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

// ───────────────────────────── Letture ─────────────────────────────

#[derive(Default)]
struct CpuSample {
    idle: u64,
    total: u64,
}

fn cpu_sample() -> Option<CpuSample> {
    let stat = read("/proc/stat")?;
    let line = stat.lines().next()?;
    let nums: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    if nums.len() < 4 {
        return None;
    }
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
    Some(CpuSample {
        idle,
        total: nums.iter().sum(),
    })
}

fn memory() -> Option<f64> {
    let info = read("/proc/meminfo")?;
    let field = |name: &str| -> Option<f64> {
        info.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
    };
    let total = field("MemTotal:")?;
    let avail = field("MemAvailable:")?;
    Some(((total - avail) / total).clamp(0.0, 1.0))
}

pub fn battery() -> Option<Battery> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in dir.flatten() {
        let p = entry.path();
        let kind = std::fs::read_to_string(p.join("type")).unwrap_or_default();
        if kind.trim() != "Battery" {
            continue;
        }
        let percent: u8 = std::fs::read_to_string(p.join("capacity"))
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let status = std::fs::read_to_string(p.join("status")).unwrap_or_default();
        let status = status.trim();
        return Some(Battery {
            percent,
            charging: status == "Charging",
            full: status == "Full",
        });
    }
    None
}

pub fn brightness() -> Option<f64> {
    let dir = std::fs::read_dir("/sys/class/backlight").ok()?;
    let p = dir.flatten().next()?.path();
    let cur: f64 = std::fs::read_to_string(p.join("brightness"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let max: f64 = std::fs::read_to_string(p.join("max_brightness"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    (max > 0.0).then(|| (cur / max).clamp(0.0, 1.0))
}

pub fn volume() -> Option<Volume> {
    // "Volume: 0.40" oppure "Volume: 0.40 [MUTED]"
    let out = output("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
    let level = out.split_whitespace().nth(1)?.parse().ok()?;
    Some(Volume {
        level,
        muted: out.contains("MUTED"),
    })
}

pub fn wifi() -> Option<Wifi> {
    let radio = output("nmcli", &["-t", "-f", "WIFI", "general"])?;
    let enabled = radio.trim() == "enabled";
    let ssid = output(
        "nmcli",
        &["-t", "-f", "NAME,TYPE", "connection", "show", "--active"],
    )
    .and_then(|s| {
        s.lines()
            .filter_map(|l| l.rsplit_once(':'))
            .find(|(_, t)| t.contains("wireless"))
            .map(|(n, _)| n.replace("\\:", ":"))
    });
    Some(Wifi { enabled, ssid })
}

pub fn bluetooth() -> Option<bool> {
    let out = output("bluetoothctl", &["show"])?;
    out.lines()
        .find(|l| l.trim_start().starts_with("Powered:"))
        .map(|l| l.contains("yes"))
}

pub fn music() -> Option<Music> {
    let out = output(
        "playerctl",
        &[
            "metadata",
            "--format",
            "{{status}}\t{{playerName}}\t{{artist}}\t{{title}}",
        ],
    )?;
    let mut parts = out.trim_end().splitn(4, '\t');
    let status = parts.next()?.to_string();
    let player = parts.next().unwrap_or_default().to_string();
    let artist = parts.next().unwrap_or_default().to_string();
    let title = parts.next().unwrap_or_default().to_string();
    if title.is_empty() && artist.is_empty() {
        return None;
    }
    Some(Music {
        playing: status == "Playing",
        player,
        artist,
        title,
    })
}

pub fn disk(path: &std::path::Path) -> Option<Disk> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` è una stringa C valida e `st` è una struttura inizializzata.
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let block = st.f_frsize as f64;
    let total = st.f_blocks as f64 * block;
    let free = st.f_bavail as f64 * block;
    if total <= 0.0 {
        return None;
    }
    let gb = 1024.0 * 1024.0 * 1024.0;
    Some(Disk {
        used: 1.0 - free / total,
        free_gb: free / gb,
        total_gb: total / gb,
    })
}

fn uptime_load() -> (u64, f64) {
    let up = read("/proc/uptime")
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
        })
        .unwrap_or(0.0) as u64;
    let load = read("/proc/loadavg")
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse().ok()))
        .unwrap_or(0.0);
    (up, load)
}

pub fn weather(location: &str) -> Option<Weather> {
    let loc = location.trim().replace(' ', "+");
    let url = format!("https://wttr.in/{loc}?format=%t|%C|%l&lang=it");
    let out = output("curl", &["-sf", "-m", "8", &url])?;
    let mut parts = out.trim().splitn(3, '|');
    let temp = parts
        .next()?
        .trim()
        .trim_end_matches('C')
        .trim_end_matches('F')
        .to_string();
    if temp.is_empty() || temp.contains("Unknown") {
        return None;
    }
    let condition = parts.next().unwrap_or_default().trim().to_string();
    let place = parts
        .next()
        .unwrap_or_default()
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    Some(Weather {
        temp,
        condition,
        place,
    })
}

// ───────────────────────────── Monitor ─────────────────────────────

pub struct MonitorOptions {
    pub weather: Option<(String, u64)>,
    pub interval: Duration,
}

/// Raccoglie i dati in un thread; `emit` viene chiamata solo quando qualcosa cambia.
pub fn spawn_monitor<F>(opts: MonitorOptions, mut emit: F)
where
    F: FnMut(SysSnapshot) + Send + 'static,
{
    let weather_slot: Arc<Mutex<Option<Weather>>> = Arc::new(Mutex::new(None));
    if let Some((location, minutes)) = opts.weather.clone() {
        if in_path("curl") {
            let slot = weather_slot.clone();
            let _ = std::thread::Builder::new()
                .name("folio-weather".into())
                .spawn(move || loop {
                    let w = weather(&location);
                    let ok = w.is_some();
                    if let Ok(mut s) = slot.lock() {
                        if ok {
                            *s = w;
                        }
                    }
                    // In caso di errore riprova prima.
                    let wait = if ok { minutes.max(5) * 60 } else { 120 };
                    std::thread::sleep(Duration::from_secs(wait));
                });
        }
    }

    let has = |p: &str| in_path(p);
    let (has_wpctl, has_nmcli, has_bt, has_playerctl) = (
        has("wpctl"),
        has("nmcli"),
        has("bluetoothctl"),
        has("playerctl"),
    );
    let home = crate::paths::home();

    let _ = std::thread::Builder::new()
        .name("folio-sys".into())
        .spawn(move || {
            let mut snap = SysSnapshot::default();
            let mut last = SysSnapshot {
                cpu: -1.0,
                ..Default::default()
            };
            let mut prev_cpu = cpu_sample().unwrap_or_default();
            let mut tick: u64 = 0;
            loop {
                if let Some(cur) = cpu_sample() {
                    let dt = cur.total.saturating_sub(prev_cpu.total) as f64;
                    let di = cur.idle.saturating_sub(prev_cpu.idle) as f64;
                    if dt > 0.0 {
                        snap.cpu = ((1.0 - di / dt) * 100.0).clamp(0.0, 100.0).round();
                    }
                    prev_cpu = cur;
                }
                snap.cpu_hist.push(snap.cpu / 100.0);
                if let Some(m) = memory() {
                    snap.mem = (m * 100.0).round();
                    snap.mem_hist.push(m);
                }
                for h in [&mut snap.cpu_hist, &mut snap.mem_hist] {
                    if h.len() > HISTORY {
                        let extra = h.len() - HISTORY;
                        h.drain(..extra);
                    }
                }
                snap.battery = battery();
                snap.brightness = brightness().map(|b| (b * 100.0).round() / 100.0);
                if has_wpctl {
                    snap.volume = volume();
                }
                if has_playerctl {
                    snap.music = music();
                }
                if tick % 3 == 0 {
                    if has_nmcli {
                        snap.wifi = wifi();
                    }
                    if has_bt {
                        snap.bluetooth = bluetooth();
                    }
                }
                if tick % 30 == 0 {
                    snap.disk = disk(&home);
                }
                let (up, load) = uptime_load();
                snap.uptime = up / 60 * 60; // alla precisione del minuto: evita aggiornamenti inutili
                snap.load = (load * 100.0).round() / 100.0;
                if let Ok(w) = weather_slot.lock() {
                    snap.weather = w.clone();
                }
                if snap != last {
                    last = snap.clone();
                    emit(snap.clone());
                }
                tick = tick.wrapping_add(1);
                std::thread::sleep(opts.interval);
            }
        });
}

// ───────────────────────────── Azioni ─────────────────────────────

pub fn set_volume(level: f64) {
    run(
        "wpctl",
        &[
            "set-volume",
            "-l",
            "1.0",
            "@DEFAULT_AUDIO_SINK@",
            &format!("{:.2}", level.clamp(0.0, 1.0)),
        ],
    );
}

pub fn toggle_mute() {
    run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]);
}

pub fn set_brightness(level: f64) {
    let pct = (level.clamp(0.02, 1.0) * 100.0).round() as u32;
    run("brightnessctl", &["-q", "set", &format!("{pct}%")]);
}

pub fn set_wifi(on: bool) {
    run("nmcli", &["radio", "wifi", if on { "on" } else { "off" }]);
}

pub fn set_bluetooth(on: bool) {
    run("bluetoothctl", &["power", if on { "on" } else { "off" }]);
}

/// "play-pause" | "next" | "previous"
pub fn media(action: &str) {
    run("playerctl", &[action]);
}

/// Invia un segnale a un processo per nome (usato per la tastiera a schermo).
pub fn signal(process: &str, sig: &str) -> bool {
    Command::new("pkill")
        .args([&format!("-{sig}"), "-x", process])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn is_running(process: &str) -> bool {
    Command::new("pgrep")
        .args(["-x", process])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
