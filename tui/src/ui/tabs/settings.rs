use crate::AppState;
use crate::ui::localization::tr;
use ac_core::config::{AppConfig, Language, PressureUnit, TempUnit};
use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsCategory {
    System,
    Display,
    RaceEngineer,
}

pub struct SettingsState {
    pub category: SettingsCategory,
    pub selected_index: usize,
    pub is_editing: bool,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            category: SettingsCategory::System,
            selected_index: 0,
            is_editing: false,
        }
    }

    pub fn next_category(&mut self) {
        self.category = match self.category {
            SettingsCategory::System => SettingsCategory::Display,
            SettingsCategory::Display => SettingsCategory::RaceEngineer,
            SettingsCategory::RaceEngineer => SettingsCategory::System,
        };
        self.selected_index = 0;
        self.is_editing = false;
    }

    pub fn prev_category(&mut self) {
        self.category = match self.category {
            SettingsCategory::System => SettingsCategory::RaceEngineer,
            SettingsCategory::Display => SettingsCategory::System,
            SettingsCategory::RaceEngineer => SettingsCategory::Display,
        };
        self.selected_index = 0;
        self.is_editing = false;
    }

    pub fn set_category(&mut self, cat: SettingsCategory) {
        self.category = cat;
        self.selected_index = 0;
        self.is_editing = false;
    }

    pub fn handle_input(&mut self, key: KeyCode, config: &mut AppConfig) {
        if !self.is_editing {
            match key {
                KeyCode::Down => self.selected_index += 1,
                KeyCode::Up => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1
                    }
                }

                KeyCode::Right => self.next_category(),
                KeyCode::Left => self.prev_category(),

                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.set_category(SettingsCategory::System)
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.set_category(SettingsCategory::Display)
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.set_category(SettingsCategory::RaceEngineer)
                }

                KeyCode::Enter => self.is_editing = true,
                _ => {}
            }

            let max_items = self.get_item_count();
            if self.selected_index >= max_items {
                self.selected_index = max_items.saturating_sub(1);
            }
        } else {
            match key {
                KeyCode::Enter | KeyCode::Esc => self.is_editing = false,
                KeyCode::Left => self.modify_value(config, -1.0),
                KeyCode::Right => self.modify_value(config, 1.0),
                KeyCode::Up => self.modify_value(config, 10.0),
                KeyCode::Down => self.modify_value(config, -10.0),
                _ => {}
            }
        }
    }

    fn get_item_count(&self) -> usize {
        match self.category {
            SettingsCategory::System => 5,
            SettingsCategory::Display => 2,
            SettingsCategory::RaceEngineer => 7,
        }
    }

    fn modify_value(&self, config: &mut AppConfig, delta: f32) {
        match self.category {
            SettingsCategory::System => match self.selected_index {
                0 => {
                    if delta > 0.0 {
                        config.language = Language::Russian;
                    } else {
                        config.language = Language::English;
                    }
                }
                1 => {
                    config.update_rate =
                        (config.update_rate as i64 + delta as i64).clamp(10, 1000) as u64
                }
                2 => {
                    config.history_size = (config.history_size as i64 + (delta * 10.0) as i64)
                        .clamp(50, 5000) as usize
                }
                3 => {
                    if delta.abs() > 0.0 {
                        config.auto_save = !config.auto_save
                    }
                }
                4 => {
                    if delta.abs() > 0.0 {
                        config.review_banner_hidden = !config.review_banner_hidden
                    }
                }
                _ => {}
            },
            SettingsCategory::Display => match self.selected_index {
                0 => {
                    if delta > 0.0 {
                        config.pressure_unit = match config.pressure_unit {
                            PressureUnit::Psi => PressureUnit::Bar,
                            PressureUnit::Bar => PressureUnit::Kpa,
                            PressureUnit::Kpa => PressureUnit::Psi,
                        };
                    } else {
                        config.pressure_unit = match config.pressure_unit {
                            PressureUnit::Psi => PressureUnit::Kpa,
                            PressureUnit::Bar => PressureUnit::Psi,
                            PressureUnit::Kpa => PressureUnit::Bar,
                        };
                    }
                }
                1 => {
                    if delta.abs() > 0.0 {
                        config.temp_unit = match config.temp_unit {
                            TempUnit::Celsius => TempUnit::Fahrenheit,
                            TempUnit::Fahrenheit => TempUnit::Celsius,
                        };
                    }
                }
                _ => {}
            },
            SettingsCategory::RaceEngineer => match self.selected_index {
                0 => {
                    config.alerts.tyre_pressure_min =
                        (config.alerts.tyre_pressure_min + delta * 0.1).max(0.0)
                }
                1 => {
                    config.alerts.tyre_pressure_max =
                        (config.alerts.tyre_pressure_max + delta * 0.1).max(0.0)
                }
                2 => config.alerts.tyre_temp_min = (config.alerts.tyre_temp_min + delta).max(0.0),
                3 => config.alerts.tyre_temp_max = (config.alerts.tyre_temp_max + delta).max(0.0),
                4 => {
                    config.alerts.brake_temp_max =
                        (config.alerts.brake_temp_max + delta * 5.0).max(0.0)
                }
                5 => {
                    config.alerts.fuel_warning_laps =
                        (config.alerts.fuel_warning_laps + delta * 0.1).max(0.0)
                }
                6 => {
                    config.alerts.wear_warning =
                        (config.alerts.wear_warning + delta).clamp(0.0, 100.0)
                }
                _ => {}
            },
        }

        if config.auto_save {
            let _res = config.save();
        }
    }

    fn get_description(&self, lang: &Language) -> String {
        let is_ru = *lang == Language::Russian;
        match self.category {
            SettingsCategory::System => match self.selected_index {
                0 => {
                    if is_ru {
                        "Язык интерфейса / Interface Language"
                    } else {
                        "Interface Language / Язык интерфейса"
                    }
                }
                1 => {
                    if is_ru {
                        "Интервал обновления телеметрии (мс). Меньше = Плавнее."
                    } else {
                        "Telemetry update rate (ms). Lower = Smoother."
                    }
                }
                2 => {
                    if is_ru {
                        "Количество точек на графиках. Больше = Длиннее история."
                    } else {
                        "Number of data points on charts. Higher = Longer history."
                    }
                }
                3 => {
                    if is_ru {
                        "Авто-сохранение настроек при выходе."
                    } else {
                        "Automatically save settings on exit."
                    }
                }
                4 => {
                    if is_ru {
                        "Показывать баннер 'Оставить отзыв' при запуске."
                    } else {
                        "Show 'Leave Review' banner on startup."
                    }
                }
                _ => "",
            },
            SettingsCategory::Display => match self.selected_index {
                0 => {
                    if is_ru {
                        "Единицы давления (PSI / Bar / kPa)."
                    } else {
                        "Pressure units (PSI / Bar / kPa)."
                    }
                }
                1 => {
                    if is_ru {
                        "Единицы температуры (Цельсий / Фаренгейт)."
                    } else {
                        "Temperature units (Celsius / Fahrenheit)."
                    }
                }
                _ => "",
            },
            SettingsCategory::RaceEngineer => match self.selected_index {
                0 => {
                    if is_ru {
                        "Мин. давление шин (Предупреждение: Синий)."
                    } else {
                        "Min Tyre Pressure (Warning: Blue)."
                    }
                }
                1 => {
                    if is_ru {
                        "Макс. давление шин (Предупреждение: Красный)."
                    } else {
                        "Max Tyre Pressure (Warning: Red)."
                    }
                }
                2 => {
                    if is_ru {
                        "Мин. температура шин (Холодные)."
                    } else {
                        "Min Tyre Temp (Cold)."
                    }
                }
                3 => {
                    if is_ru {
                        "Макс. температура шин (Перегрев)."
                    } else {
                        "Max Tyre Temp (Overheat)."
                    }
                }
                4 => {
                    if is_ru {
                        "Критическая температура тормозов."
                    } else {
                        "Critical Brake Temp."
                    }
                }
                5 => {
                    if is_ru {
                        "Остаток топлива для предупреждения (круги)."
                    } else {
                        "Fuel warning threshold (laps)."
                    }
                }
                6 => {
                    if is_ru {
                        "Критический износ шин (%)."
                    } else {
                        "Critical Tyre Wear (%)."
                    }
                }
                _ => "",
            },
        }
        .to_string()
    }
}

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;

    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)))
        .title(" CONFIGURATION TERMINAL ")
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(Color::Black));

    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);

    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(inner_area);

    let right_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(4)])
        .split(main_layout[1]);

    render_sidebar(f, main_layout[0], app);
    render_settings_list(f, right_layout[0], app);
    render_description_panel(f, right_layout[1], app);
}

fn render_sidebar(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::new(0, 1, 1, 1));

    let categories = [
        (
            SettingsCategory::System,
            if is_ru { "СИСТЕМА" } else { "SYSTEM" },
            "💻",
            "[A]",
        ),
        (
            SettingsCategory::Display,
            if is_ru { "ДИСПЛЕЙ" } else { "DISPLAY" },
            "👁️",
            "[S]",
        ),
        (
            SettingsCategory::RaceEngineer,
            if is_ru { "ИНЖЕНЕР" } else { "ENGINEER" },
            "🔧",
            "[D]",
        ),
    ];

    let items: Vec<ListItem<'_>> = categories
        .iter()
        .map(|(cat, name, icon, key)| {
            let is_selected = app.ui_state.settings.category == *cat;

            let (bg, fg, modif) = if is_selected {
                (
                    app.ui_state.get_color(&theme.highlight),
                    Color::Black,
                    Modifier::BOLD,
                )
            } else {
                (Color::Reset, Color::Gray, Modifier::empty())
            };

            let key_style = if is_selected {
                Style::default()
                    .bg(bg)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let name_span = Span::styled(
                format!(" {} {}", icon, name),
                Style::default().bg(bg).fg(fg).add_modifier(modif),
            );
            let key_span = Span::styled(format!(" {} ", key), key_style);

            let spacer = Span::styled(
                " ".repeat(area.width.saturating_sub(name.len() as u16 + 8) as usize),
                Style::default().bg(bg),
            );

            ListItem::new(Line::from(vec![name_span, spacer, key_span]))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_settings_list(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let count = app.ui_state.settings.get_item_count();
    let constraints = vec![Constraint::Length(3); count];
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    match app.ui_state.settings.category {
        SettingsCategory::System => render_system_settings(f, &rows, app),
        SettingsCategory::Display => render_display_settings(f, &rows, app),
        SettingsCategory::RaceEngineer => render_engineer_settings(f, &rows, app),
    }
}

fn render_item(
    f: &mut Frame<'_>,
    area: Rect,
    idx: usize,
    label: String,
    value: String,
    is_toggle: bool,
    app: &AppState,
) {
    let selected = idx == app.ui_state.settings.selected_index;
    let editing = app.ui_state.settings.is_editing;
    let theme = &app.ui_state.theme;

    let row_style = if selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    let block = Block::default()
        .style(row_style)
        .padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(inner);

    let label_style = if selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    f.render_widget(
        Paragraph::new(label)
            .style(label_style)
            .alignment(Alignment::Left),
        chunks[0],
    );

    let val_style = if selected && editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if selected {
        Style::default()
            .fg(app.ui_state.get_color(&theme.highlight))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let val_text = if selected && editing {
        format!("◄ {} ►", value)
    } else if is_toggle {
        let is_on = value.contains("ON") || value.contains("SHOW") || value.contains("ВКЛ");
        let box_char = if is_on { "[■]" } else { "[ ]" };
        format!("{} {}", box_char, value)
    } else if selected {
        format!("≡ {} ≡", value)
    } else {
        format!("  {}  ", value)
    };

    f.render_widget(
        Paragraph::new(val_text)
            .style(val_style)
            .alignment(Alignment::Right),
        chunks[1],
    );
}

fn render_system_settings(f: &mut Frame<'_>, areas: &[Rect], app: &AppState) {
    let config = &app.config;
    let lang = &config.language;
    let is_ru = *lang == Language::Russian;

    let lang_str = match config.language {
        Language::English => "ENGLISH",
        Language::Russian => "РУССКИЙ",
    };

    let items = vec![
        (tr("lang", lang), lang_str.to_string(), false),
        (
            tr("update_rate", lang),
            format!("{} ms", config.update_rate),
            false,
        ),
        (
            tr("history_size", lang),
            format!("{} pts", config.history_size),
            false,
        ),
        (
            tr("auto_save", lang),
            if config.auto_save {
                if is_ru { "ВКЛ" } else { "ON" }
            } else {
                if is_ru { "ВЫКЛ" } else { "OFF" }
            }
            .to_string(),
            true,
        ),
        (
            if is_ru {
                "Баннер в лаунчере"
            } else {
                "Launcher Banner"
            }
            .to_string(),
            if !config.review_banner_hidden {
                if is_ru { "ПОКАЗАТЬ" } else { "SHOW" }
            } else {
                if is_ru { "СКРЫТЬ" } else { "HIDE" }
            }
            .to_string(),
            true,
        ),
    ];

    for (i, (label, val, is_toggle)) in items.into_iter().enumerate() {
        if i < areas.len() {
            render_item(f, areas[i], i, label, val, is_toggle, app);
        }
    }
}

fn render_display_settings(f: &mut Frame<'_>, areas: &[Rect], app: &AppState) {
    let config = &app.config;
    let lang = &config.language;

    let p_unit = match config.pressure_unit {
        PressureUnit::Psi => "PSI",
        PressureUnit::Bar => "Bar",
        PressureUnit::Kpa => "kPa",
    };

    let t_unit = match config.temp_unit {
        TempUnit::Celsius => "Celsius (°C)",
        TempUnit::Fahrenheit => "Fahrenheit (°F)",
    };

    let items = vec![
        (tr("unit_pressure", lang), p_unit.to_string(), false),
        (tr("unit_temp", lang), t_unit.to_string(), false),
    ];

    for (i, (label, val, is_toggle)) in items.into_iter().enumerate() {
        if i < areas.len() {
            render_item(f, areas[i], i, label, val, is_toggle, app);
        }
    }
}

fn render_engineer_settings(f: &mut Frame<'_>, areas: &[Rect], app: &AppState) {
    let alerts = &app.config.alerts;
    let lang = &app.config.language;

    let items = vec![
        (
            tr("alert_p_min", lang),
            format!("{:.1}", alerts.tyre_pressure_min),
            false,
        ),
        (
            tr("alert_p_max", lang),
            format!("{:.1}", alerts.tyre_pressure_max),
            false,
        ),
        (
            tr("alert_t_min", lang),
            format!("{:.0}", alerts.tyre_temp_min),
            false,
        ),
        (
            tr("alert_t_max", lang),
            format!("{:.0}", alerts.tyre_temp_max),
            false,
        ),
        (
            tr("alert_b_max", lang),
            format!("{:.0}", alerts.brake_temp_max),
            false,
        ),
        (
            tr("alert_fuel", lang),
            format!("{:.1}", alerts.fuel_warning_laps),
            false,
        ),
        (
            tr("alert_wear", lang),
            format!("{:.0}%", alerts.wear_warning),
            false,
        ),
    ];

    for (i, (label, val, is_toggle)) in items.into_iter().enumerate() {
        if i < areas.len() {
            render_item(f, areas[i], i, label, val, is_toggle, app);
        }
    }
}

fn render_description_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let desc = app.ui_state.settings.get_description(&app.config.language);
    let is_ru = app.config.language == Language::Russian;

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::new(2, 2, 1, 0));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let controls_text = if is_ru {
        "[↑/↓] Выбор   [ENTER] Изменить   [←/→] Менять   [A/S/D] Категории"
    } else {
        "[↑/↓] Select   [ENTER] Edit   [←/→] Change   [A/S/D] Categories"
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let p_desc = Paragraph::new(format!("ℹ️ {}", desc)).style(Style::default().fg(Color::White));
    let p_ctrl = Paragraph::new(controls_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Right);

    f.render_widget(p_desc, chunks[0]);
    f.render_widget(p_ctrl, chunks[1]);
}
