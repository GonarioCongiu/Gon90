//! Client IPC di Hyprland.
//!
//! * `.socket.sock`  → richieste (JSON con il prefisso `j/`) e dispatch.
//! * `.socket2.sock` → flusso di eventi `EVENTO>>DATI`.
//!
//! Da Hyprland 0.55 la configurazione è in Lua e i dispatch accettano
//! espressioni `hl.dsp.*`. Folio sonda il compositor una volta sola
//! (con `hl.dsp.no_op()`) e ricade sulla sintassi classica per le versioni
//! precedenti, così lo stesso binario funziona su entrambe.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

// ───────────────────────────── Modelli ─────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct WsRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Client {
    pub address: String,
    pub mapped: bool,
    pub hidden: bool,
    pub at: [i32; 2],
    pub size: [i32; 2],
    pub workspace: WsRef,
    pub floating: bool,
    pub monitor: i64,
    pub class: String,
    pub title: String,
    pub initial_class: String,
    pub initial_title: String,
    pub pid: i64,
    pub xwayland: bool,
    pub pinned: bool,
    #[serde(deserialize_with = "de_fullscreen")]
    pub fullscreen: u8,
    #[serde(rename = "focusHistoryID")]
    pub focus_history_id: i64,
}

impl Client {
    pub fn selector(&self) -> String {
        format!("address:{}", self.address)
    }

    pub fn is_minimized(&self) -> bool {
        self.workspace.name == format!("special:{MINIMIZED}")
    }

    /// Classe da usare per abbinare la finestra a un'app installata.
    pub fn app_class(&self) -> &str {
        if self.class.is_empty() {
            &self.initial_class
        } else {
            &self.class
        }
    }
}

/// Nelle versioni recenti `fullscreen` è un intero (0/1/2), nelle vecchie un booleano.
fn de_fullscreen<'de, D: Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Bool(b) => b as u8,
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0).min(3) as u8,
        _ => 0,
    })
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub monitor: String,
    #[serde(rename = "monitorID")]
    pub monitor_id: i64,
    pub windows: u32,
    pub hasfullscreen: bool,
    pub lastwindow: String,
    pub lastwindowtitle: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: i32,
    pub focused: bool,
    pub active_workspace: WsRef,
    pub special_workspace: WsRef,
}

impl Monitor {
    /// Dimensioni logiche (quelle in cui ragionano finestre e layer-shell).
    pub fn logical_size(&self) -> (i32, i32) {
        let s = if self.scale > 0.0 { self.scale } else { 1.0 };
        let (w, h) = (
            (self.width as f64 / s).round() as i32,
            (self.height as f64 / s).round() as i32,
        );
        if self.transform % 2 == 1 {
            (h, w)
        } else {
            (w, h)
        }
    }
}

/// Spazio speciale in cui Folio "riduce a icona" le finestre
/// (Hyprland non ha un concetto nativo di minimizzazione).
pub const MINIMIZED: &str = "minimized";

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub available: bool,
    pub clients: Vec<Client>,
    pub workspaces: Vec<Workspace>,
    pub monitors: Vec<Monitor>,
    pub active: Option<Client>,
}

impl Snapshot {
    pub fn fetch() -> Self {
        if !available() {
            return Self::default();
        }
        let clients: Vec<Client> = json("clients").unwrap_or_default();
        let workspaces: Vec<Workspace> = json("workspaces").unwrap_or_default();
        let monitors: Vec<Monitor> = json("monitors").unwrap_or_default();
        // `activewindow` restituisce `{}` quando non c'è nessuna finestra attiva.
        let active = json::<Client>("activewindow").filter(|c| !c.address.is_empty());
        Self {
            available: true,
            clients,
            workspaces,
            monitors,
            active,
        }
    }

    pub fn focused_monitor(&self) -> Option<&Monitor> {
        self.monitors
            .iter()
            .find(|m| m.focused)
            .or_else(|| self.monitors.first())
    }

    pub fn active_workspace(&self) -> i64 {
        self.focused_monitor()
            .map(|m| m.active_workspace.id)
            .unwrap_or(1)
    }

    /// Spazi di lavoro "normali" (id > 0) ordinati, come le pagine di un sommario.
    pub fn normal_workspaces(&self) -> Vec<&Workspace> {
        let mut v: Vec<&Workspace> = self.workspaces.iter().filter(|w| w.id > 0).collect();
        v.sort_by_key(|w| w.id);
        v
    }

    pub fn monitor_of(&self, ws: i64) -> Option<&Monitor> {
        let name = self
            .workspaces
            .iter()
            .find(|w| w.id == ws)
            .map(|w| w.monitor.clone());
        match name {
            Some(n) => self.monitors.iter().find(|m| m.name == n),
            None => self.focused_monitor(),
        }
    }

    /// Finestre visibili di uno spazio, dalla più recente.
    pub fn windows_on(&self, ws: i64) -> Vec<&Client> {
        let mut v: Vec<&Client> = self
            .clients
            .iter()
            .filter(|c| c.workspace.id == ws && c.mapped && !c.hidden)
            .collect();
        v.sort_by_key(|c| c.focus_history_id);
        v
    }

    pub fn minimized(&self) -> Vec<&Client> {
        self.clients.iter().filter(|c| c.is_minimized()).collect()
    }

    pub fn by_address(&self, address: &str) -> Option<&Client> {
        self.clients.iter().find(|c| c.address == address)
    }

    /// Primo id libero dopo l'ultimo spazio: gli spazi sono dinamici come in GNOME.
    pub fn next_free_workspace(&self) -> i64 {
        self.normal_workspaces()
            .last()
            .map(|w| w.id + 1)
            .unwrap_or(1)
    }
}

// ───────────────────────────── Socket ─────────────────────────────

fn instance_dir() -> Option<PathBuf> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let modern = crate::paths::runtime_dir().join("hypr").join(&sig);
    if modern.exists() {
        return Some(modern);
    }
    // Hyprland < 0.40 usava /tmp/hypr.
    let legacy = PathBuf::from("/tmp/hypr").join(&sig);
    legacy.exists().then_some(legacy)
}

pub fn available() -> bool {
    instance_dir()
        .map(|d| d.join(".socket.sock").exists())
        .unwrap_or(false)
}

pub fn request(cmd: &str) -> std::io::Result<String> {
    let dir = instance_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Hyprland non in esecuzione")
    })?;
    let mut stream = UnixStream::connect(dir.join(".socket.sock"))?;
    // Hyprland serve il socket in modo sincrono: connessioni brevi e con timeout.
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(cmd.as_bytes())?;
    let mut out = String::new();
    stream.read_to_string(&mut out)?;
    Ok(out)
}

pub fn json<T: DeserializeOwned>(what: &str) -> Option<T> {
    let text = request(&format!("j/{what}")).ok()?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            log::debug!("j/{what}: {e}");
            None
        }
    }
}

/// Versione dichiarata dal compositor (`hyprctl version`).
pub fn version() -> Option<String> {
    let v: serde_json::Value = json("version")?;
    v.get("tag")
        .or_else(|| v.get("version"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
}

// ───────────────────────────── Dispatch ─────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Dispatch {
    Workspace(String),
    FocusWindow(String),
    MoveToWorkspace {
        address: String,
        workspace: String,
        follow: bool,
    },
    Close(String),
    ToggleFloat(String),
    Maximize(String),
    Fullscreen(String),
    Pin(String),
    Center(String),
    /// "left" | "right" | "up" | "down"
    MoveDirection(String),
    SendShortcut {
        address: String,
        mods: String,
        key: String,
    },
    ToggleSpecial(String),
    Exec(String),
}

/// Stringa Lua tra virgolette con gli escape necessari.
pub fn lua_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn win(address: &str) -> String {
    lua_str(&format!("address:{address}"))
}

impl Dispatch {
    pub fn lua(&self) -> String {
        match self {
            Dispatch::Workspace(ws) => format!("hl.dsp.focus({{ workspace = {} }})", lua_str(ws)),
            Dispatch::FocusWindow(a) => format!("hl.dsp.focus({{ window = {} }})", win(a)),
            Dispatch::MoveToWorkspace { address, workspace, follow } => format!(
                "hl.dsp.window.move({{ window = {}, workspace = {}, follow = {} }})",
                win(address),
                lua_str(workspace),
                follow
            ),
            Dispatch::Close(a) => format!("hl.dsp.window.close({{ window = {} }})", win(a)),
            Dispatch::ToggleFloat(a) => format!("hl.dsp.window.float({{ window = {}, action = \"toggle\" }})", win(a)),
            Dispatch::Maximize(a) => format!(
                "hl.dsp.window.fullscreen({{ window = {}, mode = \"maximized\", action = \"toggle\" }})",
                win(a)
            ),
            Dispatch::Fullscreen(a) => format!(
                "hl.dsp.window.fullscreen({{ window = {}, mode = \"fullscreen\", action = \"toggle\" }})",
                win(a)
            ),
            Dispatch::Pin(a) => format!("hl.dsp.window.pin({{ window = {}, action = \"toggle\" }})", win(a)),
            Dispatch::Center(a) => format!("hl.dsp.window.center({{ window = {} }})", win(a)),
            Dispatch::MoveDirection(d) => format!("hl.dsp.window.move({{ direction = {} }})", lua_str(d)),
            Dispatch::SendShortcut { address, mods, key } => format!(
                "hl.dsp.send_shortcut({{ window = {}, mods = {}, key = {} }})",
                win(address),
                lua_str(mods),
                lua_str(key)
            ),
            Dispatch::ToggleSpecial(name) => format!("hl.dsp.workspace.toggle_special({})", lua_str(name)),
            Dispatch::Exec(cmd) => format!("hl.dsp.exec_cmd({})", lua_str(cmd)),
        }
    }

    /// Sintassi hyprlang (Hyprland ≤ 0.54). Alcune azioni agiscono solo sulla
    /// finestra attiva, quindi prima la si mette a fuoco.
    pub fn legacy(&self) -> Vec<String> {
        let focus = |a: &str| format!("dispatch focuswindow address:{a}");
        let dir = |d: &str| match d {
            "left" => "l",
            "right" => "r",
            "up" => "u",
            _ => "d",
        };
        match self {
            Dispatch::Workspace(ws) => vec![format!("dispatch workspace {ws}")],
            Dispatch::FocusWindow(a) => vec![focus(a)],
            Dispatch::MoveToWorkspace {
                address,
                workspace,
                follow,
            } => {
                let d = if *follow {
                    "movetoworkspace"
                } else {
                    "movetoworkspacesilent"
                };
                vec![format!("dispatch {d} {workspace},address:{address}")]
            }
            Dispatch::Close(a) => vec![format!("dispatch closewindow address:{a}")],
            Dispatch::ToggleFloat(a) => vec![format!("dispatch togglefloating address:{a}")],
            Dispatch::Maximize(a) => vec![focus(a), "dispatch fullscreen 1".into()],
            Dispatch::Fullscreen(a) => vec![focus(a), "dispatch fullscreen 0".into()],
            Dispatch::Pin(a) => vec![format!("dispatch pin address:{a}")],
            Dispatch::Center(a) => vec![focus(a), "dispatch centerwindow".into()],
            Dispatch::MoveDirection(d) => vec![format!("dispatch movewindow {}", dir(d))],
            Dispatch::SendShortcut { address, mods, key } => {
                vec![format!(
                    "dispatch sendshortcut {mods},{key},address:{address}"
                )]
            }
            Dispatch::ToggleSpecial(name) => {
                vec![format!("dispatch togglespecialworkspace {name}")]
            }
            Dispatch::Exec(cmd) => vec![format!("dispatch exec {cmd}")],
        }
    }
}

const MODE_UNKNOWN: u8 = 0;
const MODE_DISPATCH_LUA: u8 = 1;
const MODE_EVAL_LUA: u8 = 2;
const MODE_LEGACY: u8 = 3;
static MODE: AtomicU8 = AtomicU8::new(MODE_UNKNOWN);

fn ok(r: std::io::Result<String>) -> bool {
    matches!(r, Ok(s) if s.trim() == "ok")
}

/// Sonda innocua: scopre quale sintassi accetta il compositor.
fn probe() -> u8 {
    let mode = if ok(request("dispatch hl.dsp.no_op()")) {
        MODE_DISPATCH_LUA
    } else if ok(request("eval hl.dispatch(hl.dsp.no_op())")) {
        MODE_EVAL_LUA
    } else {
        MODE_LEGACY
    };
    log::info!(
        "Hyprland IPC: {}",
        match mode {
            MODE_DISPATCH_LUA => "dispatch Lua",
            MODE_EVAL_LUA => "eval Lua",
            _ => "dispatch classico (hyprlang)",
        }
    );
    MODE.store(mode, Ordering::Relaxed);
    mode
}

pub fn dispatch(d: &Dispatch) -> bool {
    if !available() {
        log::debug!("dispatch ignorato (Hyprland assente): {d:?}");
        return false;
    }
    let mode = match MODE.load(Ordering::Relaxed) {
        MODE_UNKNOWN => probe(),
        m => m,
    };
    let done = match mode {
        MODE_DISPATCH_LUA => ok(request(&format!("dispatch {}", d.lua()))),
        MODE_EVAL_LUA => ok(request(&format!("eval hl.dispatch({})", d.lua()))),
        _ => d.legacy().iter().all(|c| ok(request(c))),
    };
    if !done {
        log::warn!("dispatch non riuscito: {d:?}");
    }
    done
}

/// Esegue Lua arbitrario (solo Hyprland ≥ 0.55), es. per ruotare lo schermo.
pub fn eval(lua: &str) -> bool {
    ok(request(&format!("eval {lua}")))
}

// ───────────────────────────── Eventi ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub name: String,
    pub data: String,
}

impl Event {
    pub fn parse(line: &str) -> Option<Self> {
        let (name, data) = line.split_once(">>")?;
        Some(Self {
            name: name.to_string(),
            data: data.to_string(),
        })
    }

    /// Eventi che cambiano ciò che Folio disegna (gli altri vengono ignorati).
    pub fn is_relevant(&self) -> bool {
        matches!(
            self.name.as_str(),
            "workspace"
                | "workspacev2"
                | "focusedmon"
                | "focusedmonv2"
                | "activewindow"
                | "activewindowv2"
                | "fullscreen"
                | "monitoradded"
                | "monitoraddedv2"
                | "monitorremoved"
                | "monitorremovedv2"
                | "createworkspace"
                | "createworkspacev2"
                | "destroyworkspace"
                | "destroyworkspacev2"
                | "moveworkspace"
                | "moveworkspacev2"
                | "renameworkspace"
                | "activespecial"
                | "activespecialv2"
                | "openwindow"
                | "closewindow"
                | "movewindow"
                | "movewindowv2"
                | "changefloatingmode"
                | "windowtitle"
                | "windowtitlev2"
                | "pin"
                | "urgent"
                | "minimized"
                | "configreloaded"
        )
    }
}

/// Ascolta `.socket2.sock` in un thread dedicato; si riconnette se Hyprland
/// viene riavviato.
pub fn listen<F>(mut on_event: F) -> Option<std::thread::JoinHandle<()>>
where
    F: FnMut(Event) + Send + 'static,
{
    instance_dir()?;
    let handle = std::thread::Builder::new()
        .name("hypr-events".into())
        .spawn(move || loop {
            let Some(path) = instance_dir().map(|d| d.join(".socket2.sock")) else {
                std::thread::sleep(Duration::from_secs(2));
                continue;
            };
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    let reader = BufReader::new(stream);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        if let Some(ev) = Event::parse(&line) {
                            on_event(ev);
                        }
                    }
                }
                Err(e) => log::debug!("socket2: {e}"),
            }
            std::thread::sleep(Duration::from_secs(1));
        })
        .ok()?;
    Some(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_client() {
        let j = r#"{"address":"0x5a1","mapped":true,"hidden":false,"at":[10,52],"size":[900,700],
            "workspace":{"id":2,"name":"2"},"floating":false,"monitor":0,"class":"kitty","title":"~",
            "initialClass":"kitty","initialTitle":"kitty","pid":42,"xwayland":false,"pinned":false,
            "fullscreen":2,"fullscreenClient":0,"focusHistoryID":0,"tags":[]}"#;
        let c: Client = serde_json::from_str(j).unwrap();
        assert_eq!(c.fullscreen, 2);
        assert_eq!(c.at, [10, 52]);
        assert_eq!(c.workspace.id, 2);
    }

    #[test]
    fn parses_legacy_fullscreen_bool() {
        let c: Client = serde_json::from_str(r#"{"address":"0x1","fullscreen":true}"#).unwrap();
        assert_eq!(c.fullscreen, 1);
    }

    #[test]
    fn lua_escaping() {
        let d = Dispatch::Exec("notify-send \"ciao\"".into());
        assert_eq!(d.lua(), r#"hl.dsp.exec_cmd("notify-send \"ciao\"")"#);
        let m = Dispatch::MoveToWorkspace {
            address: "0xab".into(),
            workspace: "special:minimized".into(),
            follow: false,
        };
        assert_eq!(
            m.lua(),
            r#"hl.dsp.window.move({ window = "address:0xab", workspace = "special:minimized", follow = false })"#
        );
        assert_eq!(
            m.legacy(),
            vec!["dispatch movetoworkspacesilent special:minimized,address:0xab".to_string()]
        );
    }

    #[test]
    fn event_parsing() {
        let e = Event::parse("workspacev2>>3,3").unwrap();
        assert_eq!(e.name, "workspacev2");
        assert_eq!(e.data, "3,3");
        assert!(e.is_relevant());
    }

    #[test]
    fn logical_size_with_scale_and_rotation() {
        let m = Monitor {
            width: 2560,
            height: 1600,
            scale: 1.6,
            transform: 1,
            ..Default::default()
        };
        assert_eq!(m.logical_size(), (1000, 1600));
    }
}
