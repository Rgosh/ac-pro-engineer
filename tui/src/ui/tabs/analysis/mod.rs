use crate::AppState;
use crate::ui::file_menu::FileMenu;
use crate::ui::localization::tr;
use ratatui::{prelude::*, widgets::*};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

pub mod dynamics;
pub mod engine;
pub mod graphs;
pub mod overview;
pub mod traction;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnalysisSubTab {
    Overview,
    Graphs,
    Dynamics,
    Engine,
    Traction,
}

pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// How many ticks a status message stays up. At the default 16 ms update rate
/// that is roughly three seconds — long enough to read, short enough that the
/// next message is obviously new.
const STATUS_TICKS: u16 = 200;

pub struct AnalysisState {
    pub current_tab: AnalysisSubTab,
    pub status_message: Option<String>,
    pub status_timer: u16,
    pub load_menu: RefCell<FileMenu>,
    pub loaded_file_name: Option<String>,
    pub compare_mode: bool,
    pub selected_lap_index: usize,
}

impl Default for AnalysisState {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisState {
    pub fn new() -> Self {
        Self {
            current_tab: AnalysisSubTab::Overview,
            status_message: None,
            status_timer: 0,
            load_menu: RefCell::new(FileMenu::new()),
            loaded_file_name: None,
            compare_mode: false,
            selected_lap_index: 0,
        }
    }

    pub fn next_tab(&mut self) {
        if self.load_menu.borrow().active {
            return;
        }
        self.current_tab = match self.current_tab {
            AnalysisSubTab::Overview => AnalysisSubTab::Graphs,
            AnalysisSubTab::Graphs => AnalysisSubTab::Dynamics,
            AnalysisSubTab::Dynamics => AnalysisSubTab::Engine,
            AnalysisSubTab::Engine => AnalysisSubTab::Traction,
            AnalysisSubTab::Traction => AnalysisSubTab::Overview,
        };
    }

    pub fn prev_tab(&mut self) {
        if self.load_menu.borrow().active {
            return;
        }
        self.current_tab = match self.current_tab {
            AnalysisSubTab::Overview => AnalysisSubTab::Traction,
            AnalysisSubTab::Graphs => AnalysisSubTab::Overview,
            AnalysisSubTab::Dynamics => AnalysisSubTab::Graphs,
            AnalysisSubTab::Engine => AnalysisSubTab::Dynamics,
            AnalysisSubTab::Traction => AnalysisSubTab::Engine,
        };
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_timer = STATUS_TICKS;
    }

    /// Age the current status message by one tick, clearing it when it
    /// expires.
    ///
    /// `status_timer` was set and never read by anything, so "Exported CSV:
    /// ..." and "Comparison Mode: ON" stayed pinned to the footer for the rest
    /// of the session — including after the user moved to another tab and
    /// back, which made stale messages look like fresh ones.
    pub fn tick_status(&mut self) {
        if self.status_timer > 0 {
            self.status_timer -= 1;
            if self.status_timer == 0 {
                self.status_message = None;
            }
        }
    }

    pub fn toggle_compare(&mut self) {
        if self.loaded_file_name.is_some() {
            self.compare_mode = !self.compare_mode;
            let status = if self.compare_mode { "ON" } else { "OFF" };
            self.set_status(format!("Comparison Mode: {}", status));
        } else {
            self.set_status("Load a file first to compare".to_string());
        }
    }

    pub fn save_lap_data(&mut self, lap: &ac_core::analyzer::LapData) {
        let dir = "saved_laps";
        if let Err(e) = fs::create_dir_all(dir) {
            self.set_status(format!("Error create dir: {}", e));
            return;
        }

        let clean_car = lap
            .car_model
            .replace(" ", "_")
            .replace("/", "")
            .replace("\\", "");
        let clean_track = lap
            .track_name
            .replace(" ", "_")
            .replace("/", "")
            .replace("\\", "");

        let min = lap.lap_time_ms / 60000;
        let sec = (lap.lap_time_ms % 60000) / 1000;
        let ms = lap.lap_time_ms % 1000;
        let time_str = format!("{}-{:02}-{:03}", min, sec, ms);

        let filename = format!("{}/{}_{}_{}.json", dir, clean_car, clean_track, time_str);
        let path = Path::new(&filename);

        match serde_json::to_string_pretty(lap) {
            Ok(json) => {
                if let Err(e) = fs::write(path, json) {
                    self.set_status(format!("Error saving: {}", e));
                } else {
                    self.set_status(format!("Saved: {}", filename));
                }
            }
            Err(e) => self.set_status(format!("Serialization error: {}", e)),
        }
    }

    pub fn toggle_load_menu(&mut self) {
        self.load_menu.borrow_mut().toggle();
    }

    pub fn menu_up(&mut self, _total_laps: usize) {
        if self.load_menu.borrow().active {
            self.load_menu.borrow_mut().previous();
        } else if self.selected_lap_index > 0 {
            self.selected_lap_index -= 1;
        }
    }

    pub fn menu_down(&mut self, total_laps: usize) {
        if self.load_menu.borrow().active {
            self.load_menu.borrow_mut().next();
        } else if total_laps > 0 && self.selected_lap_index + 1 < total_laps {
            self.selected_lap_index += 1;
        }
    }

    pub fn load_selected_file(&mut self, analyzer: &mut ac_core::analyzer::Analyzer) {
        let selected_file = self.load_menu.borrow().get_selected();

        if let Some(filename) = selected_file {
            let path = PathBuf::from("saved_laps").join(&filename);
            let res: Result<(), String> = (|| {
                let metadata = fs::metadata(&path).map_err(|e| format!("Read Error: {}", e))?;
                if metadata.len() > 10 * 1024 * 1024 {
                    return Err("File too large (>10MB)".to_string());
                }
                let content =
                    fs::read_to_string(&path).map_err(|e| format!("Read Error: {}", e))?;
                let mut lap = serde_json::from_str::<ac_core::analyzer::LapData>(&content)
                    .map_err(|e| format!("JSON Error: {}", e))?;

                lap.from_file = true;
                analyzer.reference_lap = Some(lap.clone());
                analyzer.laps.push(lap);
                Ok(())
            })();

            match res {
                Ok(()) => {
                    self.loaded_file_name = Some(filename.clone());
                    self.compare_mode = true;
                    self.set_status(format!("Loaded: {}", filename));
                    self.load_menu.borrow_mut().active = false;
                }
                Err(err_msg) => self.set_status(err_msg),
            }
        }
    }
}

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == ac_core::config::Language::Russian;

    let has_data = !app.analyzer.laps.is_empty();

    if !has_data {
        let block = Block::default()
            .title(tr("tab_anal", lang))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

        let msg = if is_ru {
            "Нет данных. Нажмите 'L' для загрузки или проедьте круг."
        } else {
            "No data. Press 'L' to load or drive a lap."
        };
        let text = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(text, area);
    } else {
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
            .split(area);

        render_laps_list(f, main_layout[0], app);

        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(main_layout[1]);

        render_subtabs_header(f, right_layout[0], app);

        let selected_idx = app
            .ui_state
            .analysis
            .selected_lap_index
            .min(app.analyzer.laps.len().saturating_sub(1));

        if let Some(selected_lap) = app.analyzer.laps.get(selected_idx) {
            let reference = if app.ui_state.analysis.compare_mode {
                app.analyzer.reference_lap.as_ref()
            } else {
                app.analyzer
                    .best_lap_index
                    .and_then(|i| app.analyzer.laps.get(i))
            };

            match app.ui_state.analysis.current_tab {
                AnalysisSubTab::Overview => {
                    overview::render(f, right_layout[1], app, selected_lap, reference)
                }
                AnalysisSubTab::Graphs => {
                    graphs::render(f, right_layout[1], app, selected_lap, reference)
                }
                AnalysisSubTab::Dynamics => dynamics::render(f, right_layout[1], app, selected_lap),
                AnalysisSubTab::Engine => engine::render(f, right_layout[1], app, selected_lap),
                AnalysisSubTab::Traction => traction::render(f, right_layout[1], app, selected_lap),
            }
        }

        let footer_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(right_layout[2]);

        if let Some(msg) = &app.ui_state.analysis.status_message {
            let status_p = Paragraph::new(format!("INFO: {}", msg))
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Left);
            f.render_widget(status_p, footer_layout[0]);
        }

        let hint_parts = [
            if is_ru {
                "←/→ Вкладки"
            } else {
                "←/→ Tabs"
            },
            if is_ru { "S Сохр" } else { "S Save" },
            if is_ru { "L Загр" } else { "L Load" },
            if is_ru {
                "C Сравнить"
            } else {
                "C Compare"
            },
        ];
        let hint_text = hint_parts.join(" | ");
        let hint_p = Paragraph::new(hint_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Right);

        f.render_widget(hint_p, footer_layout[1]);
    }

    if app.ui_state.analysis.load_menu.borrow().active {
        let mut menu = app.ui_state.analysis.load_menu.borrow_mut();
        crate::ui::file_menu::render(f, area, &mut menu, is_ru);
    }
}

fn render_subtabs_header(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let titles = if is_ru {
        vec!["ОБЗОР", "ТЕЛЕМЕТРИЯ", "ДИНАМИКА", "ДВИГАТЕЛЬ", "СЦЕПЛЕНИЕ"]
    } else {
        vec!["OVERVIEW", "TELEMETRY", "DYNAMICS", "ENGINE", "TRACTION"]
    };

    let selected_idx = match app.ui_state.analysis.current_tab {
        AnalysisSubTab::Overview => 0,
        AnalysisSubTab::Graphs => 1,
        AnalysisSubTab::Dynamics => 2,
        AnalysisSubTab::Engine => 3,
        AnalysisSubTab::Traction => 4,
    };

    let tabs = Tabs::new(titles)
        .select(selected_idx)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
        )
        .style(Style::default().fg(app.ui_state.get_color(&theme.text)))
        .highlight_style(
            Style::default()
                .fg(app.ui_state.get_color(&theme.highlight))
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    if let Some(fname) = &app.ui_state.analysis.loaded_file_name {
        let compare_txt = if app.ui_state.analysis.compare_mode {
            "[COMPARE]"
        } else {
            "[VIEW]"
        };
        let info = format!("{} {}", compare_txt, fname);
        let info_widget = Paragraph::new(info)
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(info_widget, area);
    }

    f.render_widget(tabs, area);
}

fn render_laps_list(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;

    let block = Block::default()
        .title(tr("anal_laps_list", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let items: Vec<ListItem<'_>> = app
        .analyzer
        .laps
        .iter()
        .enumerate()
        .map(|(i, lap)| {
            let is_best = Some(i) == app.analyzer.best_lap_index && !lap.from_file;
            let min = lap.lap_time_ms / 60000;
            let sec = (lap.lap_time_ms % 60000) / 1000;
            let ms = lap.lap_time_ms % 1000;
            let time_str = format!("{}:{:02}.{:03}", min, sec, ms);

            let mut style = Style::default().fg(app.ui_state.get_color(&theme.text));
            let content;

            if lap.from_file {
                style = style.fg(Color::Cyan);

                let car_short = if !lap.car_model.is_empty() {
                    safe_truncate(&lap.car_model, 10)
                } else {
                    "File"
                };
                content = format!("💾 {} | {}", car_short, time_str);
            } else if is_best {
                style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
                content = format!("★ L{} | {}", lap.lap_number + 1, time_str);
            } else if !lap.valid {
                style = style.fg(Color::Red);
                content = format!("L{} (X) | {}", lap.lap_number + 1, time_str);
            } else {
                content = format!("🏁 L{} | {}", lap.lap_number + 1, time_str);
            }

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(app.ui_state.get_color(&theme.highlight))
            .fg(Color::Black),
    );
    let mut state = ListState::default();
    if !app.analyzer.laps.is_empty() {
        let sel = app
            .ui_state
            .analysis
            .selected_lap_index
            .min(app.analyzer.laps.len() - 1);
        state.select(Some(sel));
    }
    f.render_stateful_widget(list, area, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_truncate_multibyte_utf8() {
        let ru_str = "Привет, мир!"; // 12 characters, >12 bytes
        assert_eq!(safe_truncate(ru_str, 6), "Привет");
        assert_eq!(safe_truncate(ru_str, 20), "Привет, мир!");

        let ascii_str = "ks_ferrari_sf70h";
        assert_eq!(safe_truncate(ascii_str, 10), "ks_ferrari");
    }

    #[test]
    fn test_analysis_lap_selection_navigation() {
        let mut state = AnalysisState::new();
        assert_eq!(state.selected_lap_index, 0);

        // Down with 5 laps
        state.menu_down(5);
        assert_eq!(state.selected_lap_index, 1);
        state.menu_down(5);
        assert_eq!(state.selected_lap_index, 2);

        // Up
        state.menu_up(5);
        assert_eq!(state.selected_lap_index, 1);

        // Up at top bounds check
        state.menu_up(5);
        assert_eq!(state.selected_lap_index, 0);
        state.menu_up(5);
        assert_eq!(state.selected_lap_index, 0);

        // Down to bottom bounds check
        state.selected_lap_index = 4;
        state.menu_down(5);
        assert_eq!(state.selected_lap_index, 4);
    }

    #[test]
    fn status_message_expires() {
        let mut state = AnalysisState::new();
        state.set_status("Exported CSV: lap3.csv".to_string());
        assert!(state.status_message.is_some());

        for _ in 0..STATUS_TICKS - 1 {
            state.tick_status();
        }
        assert!(
            state.status_message.is_some(),
            "still up one tick before expiry"
        );

        state.tick_status();
        assert!(state.status_message.is_none(), "cleared on the last tick");

        // And stays cleared rather than underflowing the counter.
        state.tick_status();
        assert_eq!(state.status_timer, 0);
    }
}
