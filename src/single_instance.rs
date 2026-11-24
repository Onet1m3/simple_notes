//! Защита от запуска второй копии: первая копия слушает Unix-сокет,
//! а повторный запуск просто просит её показать окно и завершается.

use crate::AppEvent;
use eframe::egui;
use std::{
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::mpsc::Sender,
};

fn socket_path() -> PathBuf {
    let dir = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    let user = std::env::var("USER").unwrap_or_default();
    dir.join(format!("supernote-{user}.sock"))
}

/// Если уже запущена другая копия — просит её показать окно и возвращает true.
pub fn notify_running_instance() -> bool {
    UnixStream::connect(socket_path()).is_ok()
}

/// Начинает принимать запросы от последующих запусков.
pub fn listen(events: Sender<AppEvent>, ctx: egui::Context) {
    let path = socket_path();
    // Сокет от прошлого (аварийно завершённого) запуска.
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Не удалось создать сокет {}: {e}", path.display());
            return;
        }
    };
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stream.is_ok() {
                let _ = events.send(AppEvent::Show);
                ctx.request_repaint();
            }
        }
    });
}

pub fn cleanup() {
    let _ = std::fs::remove_file(socket_path());
}
