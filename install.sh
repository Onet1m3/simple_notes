#!/usr/bin/env bash
# Сборка и установка SuperNote для текущего пользователя (без sudo).
#
#   ./install.sh                  — собрать и установить, включить автозапуск
#   ./install.sh --no-autostart   — то же, но без автозапуска
#   ./install.sh --uninstall      — удалить приложение (записи сохраняются)
set -euo pipefail

cd "$(dirname "$0")"

BIN_DIR="$HOME/.local/bin"
APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
AUTOSTART_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"

BIN="$BIN_DIR/supernote"
DESKTOP="$APPS_DIR/supernote.desktop"
ICON="$ICON_DIR/supernote.svg"
AUTOSTART="$AUTOSTART_DIR/supernote.desktop"

desktop_entry() { # $1 — дополнительные аргументы запуска
    cat <<EOF
[Desktop Entry]
Type=Application
Name=SuperNote
Comment=Блокнот с копированием записей по клику
Exec="$BIN"$1
Icon=supernote
Terminal=false
Categories=Utility;
StartupWMClass=supernote
EOF
}

if [[ "${1:-}" == "--uninstall" ]]; then
    pkill -x supernote 2>/dev/null || true
    rm -f "$BIN" "$DESKTOP" "$ICON" "$AUTOSTART"
    update-desktop-database "$APPS_DIR" 2>/dev/null || true
    echo "SuperNote удалён. Записи остались в ~/.local/share/supernote/notes.json"
    exit 0
fi

if ! command -v cargo >/dev/null; then
    echo "Не найден cargo. Установите Rust: https://rustup.rs" >&2
    exit 1
fi

echo "==> Сборка (первый раз может занять несколько минут)…"
cargo build --release

echo "==> Установка"
pkill -x supernote 2>/dev/null || true
install -Dm755 target/release/supernote "$BIN"
install -Dm644 assets/supernote.svg "$ICON"
mkdir -p "$APPS_DIR"
desktop_entry "" > "$DESKTOP"
update-desktop-database "$APPS_DIR" 2>/dev/null || true
gtk-update-icon-cache -q "${ICON_DIR%/scalable/apps}" 2>/dev/null || true

if [[ "${1:-}" == "--no-autostart" ]]; then
    rm -f "$AUTOSTART"
else
    mkdir -p "$AUTOSTART_DIR"
    { desktop_entry " --hidden"; echo "X-GNOME-Autostart-enabled=true"; } > "$AUTOSTART"
    echo "    Автозапуск при входе включён"
fi

# В GNOME значки в трее показывает расширение AppIndicator (в Ubuntu включено по умолчанию).
if command -v gnome-extensions >/dev/null \
    && ! gnome-extensions list --enabled 2>/dev/null | grep -qi appindicator; then
    echo
    echo "ВНИМАНИЕ: не включено расширение GNOME AppIndicator — значок в трее не появится."
    echo "  sudo apt install gnome-shell-extension-appindicator"
    echo "  затем перезайдите в систему и включите его в приложении «Расширения»."
fi

echo
echo "Готово. SuperNote установлен в $BIN"
echo "Запуск: найдите «SuperNote» в меню приложений или выполните: supernote"
