use ratatui::{prelude::*, widgets::*};
use crate::AppState;
use crate::config::{AppConfig, Language, PressureUnit, TempUnit};
use crate::ui::localization::tr;
use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsCategory {
    General,
    Units,
    Alerts,
}

pub struct SettingsState {
    pub category: SettingsCategory,
    pub selected_index: usize,
    pub is_editing: bool,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            category: SettingsCategory::General,
            selected_index: 0,
            is_editing: false,
        }
    }
    
    pub fn next_category(&mut self) {
        self.category = match self.category {
            SettingsCategory::General => SettingsCategory::Units,
            SettingsCategory::Units => SettingsCategory::Alerts,
            SettingsCategory::Alerts => SettingsCategory::General,
        };
        self.selected_index = 0;
        self.is_editing = false;
    }

    pub fn prev_category(&mut self) {
        self.category = match self.category {
            SettingsCategory::General => SettingsCategory::Alerts,
            SettingsCategory::Units => SettingsCategory::General,
            SettingsCategory::Alerts => SettingsCategory::Units,
        };
        self.selected_index = 0;
        self.is_editing = false;
    }
    
    pub fn handle_input(&mut self, key: KeyCode, config: &mut AppConfig) {
        if !self.is_editing {
            match key {
                KeyCode::Down => self.selected_index += 1,
                KeyCode::Up => if self.selected_index > 0 { self.selected_index -= 1 },
                KeyCode::Right | KeyCode::Tab => self.next_category(),
                KeyCode::Left => self.prev_category(),
                KeyCode::Enter => self.is_editing = true,
                _ => {}
            }
            
            let max_items = self.get_item_count(self.category);
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

    fn get_item_count(&self, category: SettingsCategory) -> usize {
        match category {
            SettingsCategory::General => 4,
            SettingsCategory::Units => 2,
            SettingsCategory::Alerts => 7,  
        }
    }

    fn modify_value(&self, config: &mut AppConfig, delta: f32) {
        match self.category {
            SettingsCategory::General => match self.selected_index {
                0 => { // Language
                    if delta > 0.0 { config.language = Language::Russian; } 
                    else { config.language = Language::English; }
                },
                1 => config.update_rate = (config.update_rate as i64 + delta as i64).clamp(1, 1000) as u64,
                2 => config.history_size = (config.history_size as i64 + (delta * 10.0) as i64).clamp(10, 2000) as usize,
                3 => if delta.abs() > 0.0 { config.auto_save = !config.auto_save },
                _ => {}
            },
            SettingsCategory::Units => match self.selected_index {
                0 => { // Pressure
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
                },
                1 => { // Temp
                     if delta.abs() > 0.0 {
                        config.temp_unit = match config.temp_unit {
                            TempUnit::Celsius => TempUnit::Fahrenheit,
                            TempUnit::Fahrenheit => TempUnit::Celsius,
                        };
                     }
                },
                _ => {}
            },
            SettingsCategory::Alerts => match self.selected_index {
                0 => config.alerts.tyre_pressure_min = (config.alerts.tyre_pressure_min + delta * 0.1).max(0.0),
                1 => config.alerts.tyre_pressure_max = (config.alerts.tyre_pressure_max + delta * 0.1).max(0.0),
                2 => config.alerts.tyre_temp_min = (config.alerts.tyre_temp_min + delta).max(0.0),
                3 => config.alerts.tyre_temp_max = (config.alerts.tyre_temp_max + delta).max(0.0),
                4 => config.alerts.brake_temp_max = (config.alerts.brake_temp_max + delta * 5.0).max(0.0),
                5 => config.alerts.fuel_warning_laps = (config.alerts.fuel_warning_laps + delta * 0.1).max(0.0),
                6 => config.alerts.wear_warning = (config.alerts.wear_warning + delta).clamp(0.0, 100.0),
                _ => {}
            },
        }
        
        if config.auto_save {
            let _ = config.save();
        }
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let main_block = Block::default()
        .title(tr("settings_title", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);
    
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25),
            Constraint::Min(0),
        ])
        .split(inner_area);
        
    render_categories_sidebar(f, layout[0], app);
    render_settings_list(f, layout[1], app);
}

fn render_categories_sidebar(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
    let categories = vec![
        (SettingsCategory::General, tr("cat_general", lang)),
        (SettingsCategory::Units, tr("cat_units", lang)),
        (SettingsCategory::Alerts, tr("cat_alerts", lang)),
    ];
    
    let items: Vec<ListItem> = categories.iter().map(|(cat, name)| {
        let is_selected = app.ui_state.settings.category == *cat;
        
        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(app.ui_state.get_color(&theme.highlight))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.ui_state.get_color(&theme.text))
        };
        
        let content = if is_selected {
            format!(" » {} ", name)
        } else {
            format!("   {} ", name)
        };
        
        ListItem::new(content).style(style)
    }).collect();
    
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_settings_list(f: &mut Frame, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);
        
    let content_area = layout[1];

    match app.ui_state.settings.category {
        SettingsCategory::General => render_general_settings(f, content_area, app),
        SettingsCategory::Units => render_units_settings(f, content_area, app),
        SettingsCategory::Alerts => render_alerts_settings(f, content_area, app),
    }
}

fn render_item_row(
    f: &mut Frame, 
    area: Rect, 
    idx: usize, 
    selected_idx: usize, 
    editing: bool, 
    label: String, 
    value: String, 
    theme: &crate::config::Theme, 
    ui_state: &crate::ui::UIState
) {
    let is_selected = idx == selected_idx;
    
    let label_style = if is_selected {
        Style::default().fg(ui_state.get_color(&theme.highlight)).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(ui_state.get_color(&theme.text))
    };
    
    let value_style = if is_selected && editing {
        Style::default().fg(Color::Black).bg(ui_state.get_color(&theme.accent))
    } else if is_selected {
        Style::default().fg(ui_state.get_color(&theme.accent)).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(ui_state.get_color(&theme.text))
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    f.render_widget(Paragraph::new(label).style(label_style), chunks[0]);
    
    let display_value = if is_selected && !editing {
        format!("‹ {} ›", value)
    } else {
        value
    };
    
    f.render_widget(Paragraph::new(display_value).style(value_style).alignment(Alignment::Right), chunks[1]);
}

fn render_general_settings(f: &mut Frame, area: Rect, app: &AppState) {
    let config = &app.config;
    let lang = &config.language;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(2); 4]) 
        .split(area);
    
    let lang_str = match config.language {
        Language::English => "English",
        Language::Russian => "Русский",
    };

    let items = vec![
        (tr("lang", lang), lang_str.to_string()),
        (tr("update_rate", lang), format!("{}", config.update_rate)),
        (tr("history_size", lang), format!("{}", config.history_size)),
        (tr("auto_save", lang), format!("{}", config.auto_save)),
    ];
    
    for (i, (label, val)) in items.into_iter().enumerate() {
        if i < layout.len() {
            render_item_row(f, layout[i], i, app.ui_state.settings.selected_index, app.ui_state.settings.is_editing, label, val, &app.ui_state.theme, &app.ui_state);
        }
    }
}

fn render_units_settings(f: &mut Frame, area: Rect, app: &AppState) {
    let config = &app.config;
    let lang = &config.language;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(2); 2]) // FIX: Исправлено кол-во на 2
        .split(area);
        
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
        (tr("unit_pressure", lang), p_unit.to_string()),
        (tr("unit_temp", lang), t_unit.to_string()),
    ];
    
    for (i, (label, val)) in items.into_iter().enumerate() {
        if i < layout.len() {
            render_item_row(f, layout[i], i, app.ui_state.settings.selected_index, app.ui_state.settings.is_editing, label, val, &app.ui_state.theme, &app.ui_state);
        }
    }
}

fn render_alerts_settings(f: &mut Frame, area: Rect, app: &AppState) {
    let alerts = &app.config.alerts;
    let lang = &app.config.language;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(2); 7])
        .split(area);
        
    let items = vec![
        (tr("alert_p_min", lang), format!("{:.1}", alerts.tyre_pressure_min)),
        (tr("alert_p_max", lang), format!("{:.1}", alerts.tyre_pressure_max)),
        (tr("alert_t_min", lang), format!("{:.0}", alerts.tyre_temp_min)),
        (tr("alert_t_max", lang), format!("{:.0}", alerts.tyre_temp_max)),
        (tr("alert_b_max", lang), format!("{:.0}", alerts.brake_temp_max)),
        (tr("alert_fuel", lang), format!("{:.1}", alerts.fuel_warning_laps)),
        (tr("alert_wear", lang), format!("{:.0}", alerts.wear_warning)),
    ];
    
    for (i, (label, val)) in items.into_iter().enumerate() {
        if i < layout.len() {
            render_item_row(f, layout[i], i, app.ui_state.settings.selected_index, app.ui_state.settings.is_editing, label, val, &app.ui_state.theme, &app.ui_state);
        }
    }
}