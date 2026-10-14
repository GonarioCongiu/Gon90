//! Socket di comando di Folio (`$XDG_RUNTIME_DIR/folio-<display>.sock`).
//!
//! Protocollo minimo: una riga di testo in ingresso, `ok` (o un errore) in uscita.
//! È ciò che usano i gesti di hyprgrass, il pulsante di hyprbars e le scorciatoie.

use crate::paths;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

#[derive(Debug)]
pub enum ServeError {
    AlreadyRunning,
    Io(std::io::Error),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::AlreadyRunning => write!(f, "Folio è già in esecuzione in questa sessione"),
            ServeError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Avvia il server in un thread. `on_command` riceve la riga e restituisce la risposta.
pub fn serve<F>(on_command: F) -> Result<(), ServeError>
where
    F: Fn(String) -> String + Send + 'static,
{
    let path = paths::socket_path();
    if UnixStream::connect(&path).is_ok() {
        return Err(ServeError::AlreadyRunning);
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(ServeError::Io)?;
    std::thread::Builder::new()
        .name("folio-ipc".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    let reply = on_command(line.trim().to_string());
                    let mut w = &stream;
                    let _ = w.write_all(reply.as_bytes());
                    let _ = w.write_all(b"\n");
                }
            }
        })
        .map_err(ServeError::Io)?;
    Ok(())
}

pub fn send(command: &str) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(paths::socket_path())?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(command.trim().as_bytes())?;
    stream.write_all(b"\n")?;
    let mut reply = String::new();
    BufReader::new(&stream).read_line(&mut reply)?;
    Ok(reply.trim().to_string())
}

pub fn cleanup() {
    let _ = std::fs::remove_file(paths::socket_path());
}
