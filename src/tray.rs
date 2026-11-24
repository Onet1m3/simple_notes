//! Значок в системном трее (Linux, протокол StatusNotifierItem через D-Bus).
//! В Ubuntu поддерживается «из коробки» расширением AppIndicator.

use crate::{autostart, icon, AppEvent};
use eframe::egui;
use ksni::blocking::TrayMethods;
use std::sync::mpsc::Sender;

struct SuperNoteTray {
    events: Sender<AppEvent>,
    ctx: egui::Context,
}

impl SuperNoteTray {
    fn send(&self, event: AppEvent) {
        let _ = self.events.send(event);
        self.ctx.request_repaint();
    }
}

impl ksni::Tray for SuperNoteTray {
    fn id(&self) -> String {
        "supernote".into()
    }

    fn title(&self) -> String {
        "SuperNote".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: icon::SIZE as i32,
            height: icon::SIZE as i32,
            data: icon::argb(),
        }]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "SuperNote".into(),
            ..Default::default()
        }
    }

    /// Левый клик по значку — показать/скрыть окно.
    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(AppEvent::Toggle);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Открыть SuperNote".into(),
                activate: Box::new(|this: &mut Self| this.send(AppEvent::Show)),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Запускать при входе в систему".into(),
                checked: autostart::is_enabled(),
                activate: Box::new(|_: &mut Self| {
                    let result = if autostart::is_enabled() {
                        autostart::disable()
                    } else {
                        autostart::enable()
                    };
                    if let Err(e) = result {
                        eprintln!("Не удалось изменить автозапуск: {e}");
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Выход".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| this.send(AppEvent::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Запускает значок в трее. Возвращает false, если трей недоступен.
pub fn spawn(events: Sender<AppEvent>, ctx: egui::Context) -> bool {
    match (SuperNoteTray { events, ctx }).spawn() {
        Ok(handle) => {
            // Держим сервис живым до конца работы программы.
            std::mem::forget(handle);
            true
        }
        Err(e) => {
            eprintln!("Трей недоступен: {e}");
            false
        }
    }
}
