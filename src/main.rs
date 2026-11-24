#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod autostart;
mod icon;
#[cfg(target_os = "linux")]
mod single_instance;
mod storage;
#[cfg(target_os = "linux")]
mod tray;

use eframe::egui;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use storage::Entry;

/// События от трея и от повторных запусков приложения.
pub enum AppEvent {
    Show,
    Toggle,
    Quit,
}

/// Сколько ждём второго клика, прежде чем считать клик одиночным.
const DOUBLE_CLICK_DELAY: Duration = Duration::from_millis(300);
const STATUS_DURATION: Duration = Duration::from_secs(2);
const BUTTONS_WIDTH: f32 = 72.0;

fn main() -> eframe::Result<()> {
    #[cfg(target_os = "linux")]
    let start_hidden = std::env::args().any(|a| a == autostart::HIDDEN_ARG);
    #[cfg(not(target_os = "linux"))]
    let start_hidden = false;

    #[cfg(target_os = "linux")]
    {
        // Уже запущено — показываем существующее окно и выходим.
        if single_instance::notify_running_instance() {
            return Ok(());
        }
        // В Wayland приложение не может скрыть своё окно (нужно для трея),
        // поэтому работаем через XWayland. Потоков ещё нет — это безопасно.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SuperNote")
            .with_app_id("supernote")
            .with_icon(egui::IconData {
                rgba: icon::rgba(),
                width: icon::SIZE,
                height: icon::SIZE,
            })
            .with_inner_size([520.0, 650.0])
            .with_min_inner_size([360.0, 400.0])
            .with_visible(!start_hidden),
        ..Default::default()
    };
    eframe::run_native(
        "SuperNote",
        options,
        Box::new(move |cc| {
            let (tx, rx) = mpsc::channel();
            #[cfg(target_os = "linux")]
            let tray_active = {
                single_instance::listen(tx.clone(), cc.egui_ctx.clone());
                tray::spawn(tx, cc.egui_ctx.clone())
            };
            #[cfg(not(target_os = "linux"))]
            let tray_active = {
                drop(tx);
                false
            };
            // Без трея окно потом не вернуть — поэтому показываем его сразу.
            if start_hidden && !tray_active {
                cc.egui_ctx
                    .send_viewport_cmd(egui::ViewportCommand::Visible(true));
            }
            Ok(Box::new(SuperNoteApp::new(
                rx,
                tray_active,
                start_hidden && tray_active,
            )))
        }),
    )
}

/// Поля формы записи.
#[derive(Default, Clone)]
struct Form {
    title: String,
    value: String,
    info: String,
    group: String,
}

impl Form {
    fn from_entry(e: &Entry) -> Self {
        Self {
            title: e.title.clone(),
            value: e.value.clone(),
            info: e.info.clone(),
            group: e.group.clone(),
        }
    }

    fn is_valid(&self) -> bool {
        !self.title.trim().is_empty() && !self.value.trim().is_empty()
    }

    fn to_entry(&self) -> Entry {
        Entry {
            title: self.title.trim().to_owned(),
            value: self.value.trim().to_owned(),
            info: self.info.trim().to_owned(),
            group: self.group.trim().to_owned(),
        }
    }

    /// Рисует поля формы. Возвращает true, если в одном из полей нажат Enter.
    /// `groups` — существующие группы для быстрого выбора.
    fn show(&mut self, ui: &mut egui::Ui, focus_first: bool, groups: &[String]) -> bool {
        let mut submitted = false;
        egui::Grid::new("entry_form")
            .num_columns(2)
            .spacing([8.0, 8.0])
            .min_col_width(80.0)
            .show(ui, |ui| {
                let fields: [(&str, &str, &mut String); 3] = [
                    ("Название*", "Например: Почта", &mut self.title),
                    ("Значение*", "Что копировать двойным кликом", &mut self.value),
                    ("Доп. инфо", "Необязательно", &mut self.info),
                ];
                for (n, (label, hint, text)) in fields.into_iter().enumerate() {
                    ui.label(label);
                    let resp = ui.add(
                        egui::TextEdit::singleline(text)
                            .hint_text(hint)
                            .desired_width(f32::INFINITY)
                            .margin(egui::vec2(6.0, 5.0)),
                    );
                    if n == 0 && focus_first {
                        resp.request_focus();
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submitted = true;
                    }
                    ui.end_row();
                }

                ui.label("Группа");
                ui.horizontal(|ui| {
                    let menu_width = if groups.is_empty() { 0.0 } else { 32.0 };
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.group)
                            .hint_text("Без группы")
                            .desired_width(ui.available_width() - menu_width)
                            .margin(egui::vec2(6.0, 5.0)),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submitted = true;
                    }
                    if !groups.is_empty() {
                        ui.menu_button("▾", |ui| {
                            if ui.button("Без группы").clicked() {
                                self.group.clear();
                                ui.close();
                            }
                            ui.separator();
                            for g in groups {
                                if ui.button(g).clicked() {
                                    self.group = g.clone();
                                    ui.close();
                                }
                            }
                        })
                        .response
                        .on_hover_text("Выбрать существующую группу");
                    }
                });
                ui.end_row();
            });
        submitted
    }
}

/// Открытое окно добавления (`index == None`) или редактирования записи.
struct Dialog {
    index: Option<usize>,
    form: Form,
    focus_requested: bool,
}

enum DialogResult {
    Save,
    Cancel,
}

struct SuperNoteApp {
    entries: Vec<Entry>,
    search: String,
    dialog: Option<Dialog>,
    /// Одиночный клик, ожидающий подтверждения (не стал ли он двойным).
    pending_click: Option<(usize, Instant)>,
    status: Option<(String, Instant)>,
    events: Receiver<AppEvent>,
    /// Есть значок в трее: закрытие окна сворачивает приложение, а не завершает.
    tray_active: bool,
    hidden: bool,
    quitting: bool,
}

impl SuperNoteApp {
    fn set_visible(&mut self, ctx: &egui::Context, visible: bool) {
        self.hidden = !visible;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
        if visible {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn new(events: Receiver<AppEvent>, tray_active: bool, hidden: bool) -> Self {
        let (entries, status) = match storage::load() {
            Ok(entries) => (entries, None),
            Err(e) => (Vec::new(), Some((format!("Ошибка загрузки: {e}"), Instant::now()))),
        };
        Self {
            entries,
            search: String::new(),
            dialog: None,
            pending_click: None,
            status,
            events,
            tray_active,
            hidden,
            quitting: false,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), Instant::now()));
    }

    fn save(&mut self) {
        if let Err(e) = storage::save(&self.entries) {
            self.set_status(format!("Ошибка сохранения: {e}"));
        }
    }

    fn open_dialog(&mut self, index: Option<usize>) {
        let form = index
            .and_then(|i| self.entries.get(i))
            .map(Form::from_entry)
            .unwrap_or_default();
        self.dialog = Some(Dialog {
            index,
            form,
            focus_requested: false,
        });
    }

    fn commit_dialog(&mut self) {
        let Some(dialog) = &self.dialog else { return };
        if !dialog.form.is_valid() {
            self.set_status("Заполните название и значение");
            return;
        }
        let entry = dialog.form.to_entry();
        match dialog.index {
            Some(i) if i < self.entries.len() => {
                self.entries[i] = entry;
                self.set_status("Запись сохранена");
            }
            _ => {
                self.entries.push(entry);
                self.set_status("Запись добавлена");
            }
        }
        self.dialog = None;
        self.save();
    }

    fn delete_entry(&mut self, index: usize) {
        if index >= self.entries.len() {
            return;
        }
        self.entries.remove(index);
        self.pending_click = None;
        self.save();
        self.set_status("Запись удалена");
    }

    fn copy(&mut self, ctx: &egui::Context, text: String, what: &str) {
        self.set_status(format!("Скопировано ({what}): {text}"));
        ctx.copy_text(text);
    }

    /// Индексы записей, подходящих под поисковый запрос (без учёта регистра).
    fn filtered_indices(&self) -> Vec<usize> {
        let query = self.search.trim().to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                query.is_empty()
                    || [&e.title, &e.value, &e.info, &e.group]
                        .iter()
                        .any(|f| f.to_lowercase().contains(&query))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Все непустые группы по алфавиту (без учёта регистра).
    fn group_names(&self) -> Vec<String> {
        let mut groups: Vec<String> = self
            .entries
            .iter()
            .filter(|e| !e.group.is_empty())
            .map(|e| e.group.clone())
            .collect();
        groups.sort_by_key(|g| g.to_lowercase());
        groups.dedup();
        groups
    }
}

enum Action {
    Click(usize),
    DoubleClick(usize),
    Edit(usize),
    Delete(usize),
}

impl eframe::App for SuperNoteApp {
    /// Вызывается каждый кадр, даже когда окно скрыто.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // eframe принудительно показывает окно после первого кадра,
        // поэтому при запуске в трей скрываем его повторно на первых кадрах.
        if self.hidden && ctx.cumulative_frame_nr() < 3 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            ctx.request_repaint();
        }

        while let Ok(event) = self.events.try_recv() {
            match event {
                AppEvent::Show => self.set_visible(ctx, true),
                AppEvent::Toggle => {
                    let hidden = self.hidden;
                    self.set_visible(ctx, hidden);
                }
                AppEvent::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        // Крестик окна сворачивает в трей вместо выхода.
        if self.tray_active && !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = None;
            self.set_visible(ctx, false);
        }
    }

    fn on_exit(&mut self) {
        #[cfg(target_os = "linux")]
        single_instance::cleanup();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        // Чтобы текст записей не перехватывал клики выделением.
        ui.style_mut().interaction.selectable_labels = false;

        // Одиночный клик срабатывает, только если второй клик так и не пришёл.
        if let Some((i, t)) = self.pending_click {
            if t.elapsed() >= DOUBLE_CLICK_DELAY {
                self.pending_click = None;
                if let Some(e) = self.entries.get(i) {
                    let title = e.title.clone();
                    self.copy(ctx, title, "название");
                }
            } else {
                ctx.request_repaint_after(DOUBLE_CLICK_DELAY - t.elapsed());
            }
        }

        let dialog_open = self.dialog.is_some();
        let mut action: Option<Action> = None;

        // Поиск и кнопка добавления.
        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.add_enabled_ui(!dialog_open, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let add_width = 110.0;
                    let clear_width = if self.search.is_empty() { 0.0 } else { 32.0 };
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("🔍 Поиск по записям…")
                            .desired_width(ui.available_width() - add_width - clear_width - 8.0)
                            .margin(egui::vec2(6.0, 5.0)),
                    );
                    if !self.search.is_empty()
                        && ui.button("✖").on_hover_text("Очистить поиск").clicked()
                    {
                        self.search.clear();
                    }
                    if ui
                        .add(egui::Button::new("➕ Добавить").min_size(egui::vec2(add_width, 28.0)))
                        .clicked()
                    {
                        self.open_dialog(None);
                    }
                });
                ui.add_space(8.0);
            });
        });

        // Строка статуса.
        egui::Panel::bottom("status_panel").show(ui, |ui| {
            let text = match &self.status {
                Some((msg, t)) if t.elapsed() < STATUS_DURATION => {
                    ctx.request_repaint_after(Duration::from_millis(200));
                    msg.clone()
                }
                _ => format!(
                    "Записей: {} — клик: копировать название, двойной клик: значение",
                    self.entries.len()
                ),
            };
            ui.label(text);
        });

        let visible = self.filtered_indices();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_enabled_ui(!dialog_open, |ui| {
                if visible.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.weak(if self.entries.is_empty() {
                            "Пока нет записей — нажмите «➕ Добавить»"
                        } else {
                            "Ничего не найдено"
                        });
                    });
                    return;
                }

                let groups = self.group_names();
                let searching = !self.search.trim().is_empty();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Нет ни одной группы — показываем простой список.
                        if groups.is_empty() {
                            for &i in &visible {
                                show_entry_card(ui, i, &self.entries[i], &mut action);
                                ui.add_space(4.0);
                            }
                            return;
                        }

                        // Сначала именованные группы, в конце — записи без группы.
                        let sections = groups
                            .iter()
                            .map(|g| (g.as_str(), g.as_str()))
                            .chain(std::iter::once(("", "Без группы")));
                        for (key, name) in sections {
                            let items: Vec<usize> = visible
                                .iter()
                                .copied()
                                .filter(|&i| self.entries[i].group == key)
                                .collect();
                            if items.is_empty() {
                                continue;
                            }
                            let mut header = egui::CollapsingHeader::new(
                                egui::RichText::new(format!("📁 {name}  ({})", items.len()))
                                    .size(15.0)
                                    .strong(),
                            )
                            .id_salt(("group", key))
                            .default_open(true);
                            // Во время поиска раскрываем группы с найденными записями.
                            if searching {
                                header = header.open(Some(true));
                            }
                            header.show(ui, |ui| {
                                for &i in &items {
                                    show_entry_card(ui, i, &self.entries[i], &mut action);
                                    ui.add_space(4.0);
                                }
                            });
                            ui.add_space(4.0);
                        }
                    });
            });
        });

        match action {
            Some(Action::Click(i)) => self.pending_click = Some((i, Instant::now())),
            Some(Action::DoubleClick(i)) => {
                self.pending_click = None;
                let value = self.entries[i].value.clone();
                self.copy(ctx, value, "значение");
            }
            Some(Action::Edit(i)) => self.open_dialog(Some(i)),
            Some(Action::Delete(i)) => self.delete_entry(i),
            None => {}
        }

        // Отдельное окно добавления/редактирования.
        let groups = self.group_names();
        if let Some(dialog) = &mut self.dialog {
            match show_dialog(ctx, dialog, &groups) {
                Some(DialogResult::Save) => self.commit_dialog(),
                Some(DialogResult::Cancel) => self.dialog = None,
                None => {}
            }
        }
    }
}

fn show_dialog(
    ctx: &egui::Context,
    dialog: &mut Dialog,
    groups: &[String],
) -> Option<DialogResult> {
    let title = if dialog.index.is_some() {
        "Редактирование записи"
    } else {
        "Новая запись"
    };
    let builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size([440.0, 230.0])
        .with_min_inner_size([360.0, 230.0])
        .with_window_level(egui::WindowLevel::AlwaysOnTop);

    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("entry_dialog"),
        builder,
        |ui, class| {
            let mut result = None;
            let mut contents = |ui: &mut egui::Ui| {
                ui.style_mut().interaction.selectable_labels = false;
                let focus = !dialog.focus_requested;
                dialog.focus_requested = true;
                let submitted = dialog.form.show(ui, focus, groups);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let valid = dialog.form.is_valid();
                    let save_label = if dialog.index.is_some() {
                        "✔ Сохранить"
                    } else {
                        "➕ Добавить"
                    };
                    let save = ui.add_enabled(
                        valid,
                        egui::Button::new(save_label).min_size(egui::vec2(110.0, 28.0)),
                    );
                    if save.clicked() || (submitted && valid) {
                        result = Some(DialogResult::Save);
                    }
                    if ui
                        .add(egui::Button::new("Отмена").min_size(egui::vec2(80.0, 28.0)))
                        .clicked()
                    {
                        result = Some(DialogResult::Cancel);
                    }
                    ui.weak("* обязательные поля");
                });
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = Some(DialogResult::Cancel);
                }
            };

            if class == egui::ViewportClass::EmbeddedWindow {
                // Бэкенд не поддерживает отдельные окна — показываем внутри основного.
                contents(ui);
            } else {
                egui::CentralPanel::default().show(ui, |ui| contents(ui));
            }

            // Закрытие окна крестиком = отмена.
            if ui.input(|i| i.viewport().close_requested()) {
                result = Some(DialogResult::Cancel);
            }
            result
        },
    )
}

fn show_entry_card(ui: &mut egui::Ui, i: usize, entry: &Entry, action: &mut Option<Action>) {
    ui.horizontal(|ui| {
        let card_width = (ui.available_width() - BUTTONS_WIDTH).max(100.0);
        let card = egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_width(card_width - 22.0);
                ui.vertical(|ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(&entry.title).size(16.0).strong())
                            .truncate(),
                    );
                    ui.add(egui::Label::new(egui::RichText::new(&entry.value).weak()).truncate());
                    if !entry.info.is_empty() {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&entry.info).small().italics())
                                .truncate(),
                        );
                    }
                });
            });

        let resp = ui
            .interact(card.response.rect, ui.scope_id().with(("entry", i)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Клик — копировать название\nДвойной клик — копировать значение");
        if resp.hovered() {
            ui.painter().rect_stroke(
                card.response.rect,
                4.0,
                ui.visuals().widgets.hovered.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        if resp.double_clicked() {
            *action = Some(Action::DoubleClick(i));
        } else if resp.clicked() {
            *action = Some(Action::Click(i));
        }

        if ui.button("✏").on_hover_text("Редактировать").clicked() {
            *action = Some(Action::Edit(i));
        }
        if ui.button("🗑").on_hover_text("Удалить").clicked() {
            *action = Some(Action::Delete(i));
        }
    });
}
