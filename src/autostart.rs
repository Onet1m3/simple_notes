//! Автозапуск при входе в систему через ~/.config/autostart/supernote.desktop
//! (стандарт freedesktop, работает в GNOME/Ubuntu, KDE, XFCE и др.).

use std::{fs, io, path::PathBuf};

/// Аргумент командной строки: запуститься свёрнутым в трей.
pub const HIDDEN_ARG: &str = "--hidden";

fn desktop_file() -> io::Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "не найден каталог настроек"))?;
    Ok(dir.join("autostart").join("supernote.desktop"))
}

pub fn is_enabled() -> bool {
    desktop_file().is_ok_and(|p| p.exists())
}

pub fn enable() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let path = desktop_file()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let exe = exe.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=SuperNote\n\
         Comment=Блокнот с копированием записей по клику\n\
         Exec=\"{exe}\" {HIDDEN_ARG}\n\
         Icon=supernote\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    );
    fs::write(path, content)
}

pub fn disable() -> io::Result<()> {
    match fs::remove_file(desktop_file()?) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
