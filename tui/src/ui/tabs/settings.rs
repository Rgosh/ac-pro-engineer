use crate::AppState;
use crate::ui::localization::tr;
use ac_core::config::{AppConfig, Language, PressureUnit, TempUnit};
use ac_core::i18n::{Translate, tr_fmt};
use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsCategory {
    System,
    Display,
    RaceEngineer,
    Overlay,
    Keys,
}

pub struct SettingsState {
    pub category: SettingsCategory,
    pub selected_index: usize,
    pub is_editing: bool,
    /// Waiting for the next keypress to become a binding.
    ///
    /// While this is set the main loop hands over the key untouched, before
    /// resolving it — otherwise the overlay toggle could never be rebound,
    /// because pressing it would toggle the overlay on the way past.
    pub capturing: bool,
    /// What the last capture refused to do, and why.
    pub key_message: Option<String>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            category: SettingsCategory::System,
            selected_index: 0,
            is_editing: false,
            capturing: false,
            key_message: None,
        }
    }

    pub fn next_category(&mut self) {
        self.category = match self.category {
            SettingsCategory::System => SettingsCategory::Display,
            SettingsCategory::Display => SettingsCategory::RaceEngineer,
            SettingsCategory::RaceEngineer => SettingsCategory::Overlay,
            SettingsCategory::Overlay => SettingsCategory::Keys,
            SettingsCategory::Keys => SettingsCategory::System,
        };
        self.selected_index = 0;
        self.is_editing = false;
        self.capturing = false;
    }

    pub fn prev_category(&mut self) {
        self.category = match self.category {
            SettingsCategory::System => SettingsCategory::Keys,
            SettingsCategory::Display => SettingsCategory::System,
            SettingsCategory::RaceEngineer => SettingsCategory::Display,
            SettingsCategory::Overlay => SettingsCategory::RaceEngineer,
            SettingsCategory::Keys => SettingsCategory::Overlay,
        };
        self.selected_index = 0;
        self.is_editing = false;
        self.capturing = false;
    }

    pub fn set_category(&mut self, cat: SettingsCategory) {
        self.category = cat;
        self.selected_index = 0;
        self.is_editing = false;
        self.capturing = false;
    }

    /// Take a keypress as the new binding for the selected action.
    ///
    /// Returns whether the config changed, so the caller writes it out. Esc
    /// cancels; Delete puts the default back. A key already taken by another
    /// action is refused with a reason rather than written — two actions on one
    /// key means one of them silently stops working, and there is nothing on
    /// screen to say which.
    pub fn capture_key(&mut self, key: crossterm::event::KeyEvent, config: &mut AppConfig) -> bool {
        use crossterm::event::KeyCode;

        self.capturing = false;

        if key.code == KeyCode::Esc {
            self.key_message = None;
            return false;
        }

        let bindings = crate::keys::all(&config.keys);
        let Some((field, label, _)) = bindings.get(self.selected_index).copied() else {
            return false;
        };

        if key.code == KeyCode::Delete || key.code == KeyCode::Backspace {
            let default = crate::keys::all(&ac_core::config::KeyBindings::default())
                .into_iter()
                .find(|(name, _, _)| *name == field)
                .map(|(_, _, value)| value.to_string());
            if let Some(default) = default {
                crate::keys::set(&mut config.keys, field, default);
                self.key_message = None;
                return true;
            }
            return false;
        }

        let Some(spelled) = crate::keys::spell(key) else {
            self.key_message = Some("That key cannot be bound".to_string());
            return false;
        };

        if let Some(taken_by) = crate::keys::conflict(&config.keys, field, &spelled) {
            self.key_message = Some(format!(
                "{} is already {}",
                crate::keys::describe(&spelled),
                taken_by
            ));
            return false;
        }

        crate::keys::set(&mut config.keys, field, spelled.clone());
        self.key_message = Some(format!(
            "{label} is now {}",
            crate::keys::describe(&spelled)
        ));
        true
    }

    /// Returns whether this keypress changed `config`, so the caller knows to
    /// persist it. Navigating between items and categories does not; only the
    /// editing branch below touches the config.
    pub fn handle_input(&mut self, key: KeyCode, config: &mut AppConfig) -> bool {
        // Bindings are not numbers, so the left/right/edit machinery below
        // does not apply: Enter arms the capture, and the next keypress goes
        // to `capture_key` before anything else looks at it.
        if self.category == SettingsCategory::Keys {
            match key {
                KeyCode::Down => self.selected_index += 1,
                KeyCode::Up => self.selected_index = self.selected_index.saturating_sub(1),
                KeyCode::Right => self.next_category(),
                KeyCode::Left => self.prev_category(),
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.set_category(SettingsCategory::System)
                }
                KeyCode::Enter => {
                    self.capturing = true;
                    self.key_message = None;
                }
                _ => {}
            }
            let max_items = self.get_item_count();
            if self.selected_index >= max_items {
                self.selected_index = max_items.saturating_sub(1);
            }
            return false;
        }

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
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    self.set_category(SettingsCategory::Overlay)
                }
                KeyCode::Char('g') | KeyCode::Char('G') => {
                    self.set_category(SettingsCategory::Keys)
                }

                KeyCode::Enter => self.is_editing = true,
                _ => {}
            }

            let max_items = self.get_item_count();
            if self.selected_index >= max_items {
                self.selected_index = max_items.saturating_sub(1);
            }
            false
        } else {
            match key {
                KeyCode::Enter | KeyCode::Esc => {
                    self.is_editing = false;
                    false
                }
                KeyCode::Left => {
                    self.modify_value(config, -1.0);
                    true
                }
                KeyCode::Right => {
                    self.modify_value(config, 1.0);
                    true
                }
                KeyCode::Up => {
                    self.modify_value(config, 10.0);
                    true
                }
                KeyCode::Down => {
                    self.modify_value(config, -10.0);
                    true
                }
                _ => false,
            }
        }
    }

    fn get_item_count(&self) -> usize {
        match self.category {
            SettingsCategory::System => 5,
            SettingsCategory::Display => 2,
            SettingsCategory::RaceEngineer => 11,
            SettingsCategory::Overlay => 7,
            // Counted off the binding list rather than written down, so adding
            // an action cannot leave a row that is drawn and unreachable.
            SettingsCategory::Keys => {
                crate::keys::all(&ac_core::config::KeyBindings::default()).len()
            }
        }
    }

    fn modify_value(&self, config: &mut AppConfig, delta: f32) {
        match self.category {
            // Bindings are captured, not nudged: see `capture_key`.
            SettingsCategory::Keys => {}
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
                4 if delta.abs() > 0.0 => {
                    config.review_banner_hidden = !config.review_banner_hidden
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
                1 if delta.abs() > 0.0 => {
                    config.temp_unit = match config.temp_unit {
                        TempUnit::Celsius => TempUnit::Fahrenheit,
                        TempUnit::Fahrenheit => TempUnit::Celsius,
                    };
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
                        (config.alerts.wear_warning + delta * 0.5).clamp(0.0, 100.0)
                }
                7 => {
                    config.alerts.wear_critical =
                        (config.alerts.wear_critical + delta * 0.5).clamp(0.0, 100.0)
                }
                8 => {
                    config.target_hot_pressure_front =
                        (config.target_hot_pressure_front + delta * 0.1).clamp(15.0, 45.0)
                }
                9 => {
                    config.target_hot_pressure_rear =
                        (config.target_hot_pressure_rear + delta * 0.1).clamp(15.0, 45.0)
                }
                10 if delta.abs() > 0.0 => config.show_ghost_delta = !config.show_ghost_delta,
                _ => {}
            },
            SettingsCategory::Overlay => match self.selected_index {
                0 if delta.abs() > 0.0 => {
                    config.overlay.show_telemetry = !config.overlay.show_telemetry
                }
                1 if delta.abs() > 0.0 => {
                    config.overlay.show_engineer = !config.overlay.show_engineer
                }
                2 if delta.abs() > 0.0 => {
                    config.overlay.show_session = !config.overlay.show_session
                }
                3 if delta.abs() > 0.0 => config.overlay.show_timing = !config.overlay.show_timing,
                4 if delta.abs() > 0.0 => config.overlay.show_fuel = !config.overlay.show_fuel,
                5 => {
                    let next = config.overlay.engineer_lines as i32 + delta.signum() as i32;
                    // The frame's slot count, not a literal: this used to say
                    // 4 in one place and MESSAGE_SLOTS in another, and the
                    // setting silently refused to go past the older number.
                    let slots = ac_core::overlay::frame::MESSAGE_SLOTS as i32;
                    config.overlay.engineer_lines = next.clamp(0, slots) as u8;
                }
                6 if delta.abs() > 0.0 => {
                    config.overlay.startup_card = !config.overlay.startup_card
                }
                _ => {}
            },
        }
    }

    fn get_description(&self, lang: &Language) -> String {
        let is_ru = *lang == Language::Russian;
        match self.category {
            SettingsCategory::Keys => {
                // One line, and it has to fit: the description pane does not
                // wrap, so a longer sentence comes out with a hole in it.
                return "ENTER to bind, DEL for the default, ESC to cancel"
                    .tr(is_ru)
                    .to_string();
            }
            SettingsCategory::System => match self.selected_index {
                0 => {
                    if is_ru {
                        "Язык интерфейса / Interface Language"
                    } else {
                        "Interface Language / Язык интерфейса"
                    }
                }
                1 => "Telemetry update rate (ms). Lower = Smoother.".tr(is_ru),
                2 => "Number of data points on charts. Higher = Longer history.".tr(is_ru),
                3 => "Automatically save settings on exit.".tr(is_ru),
                4 => "Show 'Leave Review' banner on startup.".tr(is_ru),
                _ => "",
            },
            SettingsCategory::Display => match self.selected_index {
                0 => "Pressure units (PSI / Bar / kPa).".tr(is_ru),
                1 => "Temperature units (Celsius / Fahrenheit).".tr(is_ru),
                _ => "",
            },
            SettingsCategory::RaceEngineer => match self.selected_index {
                0 => "Min Tyre Pressure (Warning: Blue).".tr(is_ru),
                1 => "Max Tyre Pressure (Warning: Red).".tr(is_ru),
                2 => "Min Tyre Temp (Cold).".tr(is_ru),
                3 => "Max Tyre Temp (Overheat).".tr(is_ru),
                4 => "Critical Brake Temp.".tr(is_ru),
                5 => "Fuel warning threshold (laps).".tr(is_ru),
                6 => "Tyre life below which it is a warning (%).".tr(is_ru),
                7 => "Tyre life below which it is critical (%).".tr(is_ru),
                8 => "Target hot pressure, front.".tr(is_ru),
                9 => "Target hot pressure, rear.".tr(is_ru),
                10 => "Measure the delta against your own best lap, not AC's meter.".tr(is_ru),
                _ => "",
            },
            SettingsCategory::Overlay => match self.selected_index {
                0 => "Show the telemetry block in the in-game overlay.".tr(is_ru),
                1 => "Show engineer advice in the in-game overlay.".tr(is_ru),
                2 => "Show position, lap and track conditions in the overlay.".tr(is_ru),
                3 => "Show delta and lap times in the overlay.".tr(is_ru),
                4 => "Show fuel and remaining laps in the overlay.".tr(is_ru),
                5 => "How many engineer lines reach the overlay (0-8). The \
                         panel may draw fewer — it has a slider of its own."
                    .tr(is_ru),
                6 => "The startup card. [I] installs it, [U] removes it from the game.".tr(is_ru),
                _ => "",
            },
        }
        .to_string()
    }
}

/// Ask before touching the game folder.
pub fn render_confirm_popup(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let Some(action) = app.overlay_confirm else {
        return;
    };

    let width = 64.min(area.width.saturating_sub(4));
    let height = 11;
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);

    let is_ru = app.config.language == Language::Russian;
    let removing = action == crate::OverlayAction::Uninstall;

    let colour = if removing {
        Color::Yellow
    } else {
        Color::Green
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().fg(colour).bg(Color::Black))
        .title(if removing {
            " REMOVE THE OVERLAY? ".tr(is_ru)
        } else {
            " INSTALL THE OVERLAY? ".tr(is_ru)
        })
        .title_alignment(Alignment::Center);

    let dim = Style::default().fg(Color::DarkGray);
    let white = Style::default().fg(Color::White);

    let mut lines = vec![Line::from("")];
    lines.push(Line::from(Span::styled(
        if removing {
            "  The panel's files leave the game folder.".tr(is_ru)
        } else {
            "  The panel's files go into the game folder.".tr(is_ru)
        },
        white,
    )));

    // The count, from the installer rather than from a word in a sentence.
    // This said "four" in five places, and the panel is nineteen files now.
    lines.push(Line::from(Span::styled(
        format!(
            "  {} {}",
            ac_core::overlay::install::file_count(),
            "files".tr(is_ru)
        ),
        dim,
    )));

    if let Some(path) = app.overlay_report.app_path.as_ref() {
        let text = path.display().to_string();
        let shown = if text.len() > (width as usize).saturating_sub(6) {
            format!("…{}", &text[text.len() - (width as usize - 7)..])
        } else {
            text
        };
        lines.push(Line::from(Span::styled(format!("  {shown}"), dim)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  The panel's settings live elsewhere and are not touched.".tr(is_ru),
        dim,
    )));
    lines.push(Line::from(""));

    let yes = if app.overlay_confirm_selection == 0 {
        Style::default()
            .fg(Color::Black)
            .bg(colour)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let no = if app.overlay_confirm_selection == 1 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    lines.push(Line::from(vec![
        Span::raw("      "),
        Span::styled(
            if removing {
                " [ YES, REMOVE ] ".tr(is_ru)
            } else {
                " [ YES, INSTALL ] ".tr(is_ru)
            },
            yes,
        ),
        Span::raw("     "),
        Span::styled(" [ CANCEL ] ".tr(is_ru), no),
    ]));

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left),
        popup,
    );
}

/// What the last install or removal did, over the settings that asked for it.
/// The whole bridge report, full screen.
///
/// Everything the application knows about whether the overlay can work, which
/// until now lived only in `cargo run -p ac_core --example bridge_probe` — a
/// command someone who downloaded a release cannot run and has no reason to
/// know about, answering the single most common question about this program.
pub fn render_diagnosis(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    use ac_core::overlay::diagnosis::Tone;

    let report = &app.overlay_diagnosis;

    let width = 84.min(area.width.saturating_sub(2));
    // Sized to what it has to say. A box that is always full height leaves
    // half a screen of empty border under a six-line answer, which reads as
    // something having failed to load.
    let content = report
        .lines
        .iter()
        .map(|line| {
            // A heading costs a blank line above it, and anything longer than
            // the box wraps onto more.
            let text = line.label.len() + line.value.len() + 6;
            let wrapped = text.div_ceil((width as usize).max(1));
            wrapped.max(1) + usize::from(line.tone == Tone::Heading)
        })
        .sum::<usize>()
        + 6; // the verdict, the footer, and the blank lines around them
    let height = (content as u16 + 2).min(area.height.saturating_sub(2));

    let popup = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);
    let is_ru = app.config.language == Language::Russian;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().fg(if report.workable {
            Color::Green
        } else {
            Color::Red
        }))
        .title(" OVERLAY DIAGNOSTICS ".tr(is_ru))
        .title_alignment(Alignment::Center);

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(report.lines.len() + 4);
    for entry in &report.lines {
        match entry.tone {
            Tone::Heading => {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    entry.value.to_uppercase(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            Tone::Action => lines.push(Line::from(Span::styled(
                format!("    {}", entry.value),
                Style::default().fg(Color::Yellow),
            ))),
            tone => {
                let colour = match tone {
                    Tone::Good => Color::Green,
                    Tone::Warn => Color::Yellow,
                    Tone::Bad => Color::Red,
                    _ => Color::White,
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {:<17}", entry.label),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(entry.value.clone(), Style::default().fg(colour)),
                ]));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  {} {}", "OVERLAY".tr(is_ru), report.verdict),
        Style::default()
            .fg(if report.workable {
                Color::Green
            } else {
                Color::Red
            })
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  [R] check again   ESC to close".tr(is_ru),
        Style::default().fg(Color::DarkGray),
    )));

    // Wrapped: the paths and the explanations are longer than any width this
    // is likely to get, and a clipped path is the one thing on this screen
    // nobody can act on.
    f.render_widget(
        Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }),
        inner,
    );
}

pub fn render_result_popup(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let width = 62.min(area.width.saturating_sub(4));
    let height = 9;
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);

    let removed = app.overlay_install_status.starts_with("removed")
        || app.overlay_install_status.starts_with("nothing to remove");
    let colour = if app.overlay_install_status.contains("could not")
        || app.overlay_install_status.contains("no Assetto")
    {
        Color::Red
    } else if removed {
        Color::Yellow
    } else {
        Color::Green
    };

    let is_ru = app.config.language == Language::Russian;
    let title = if removed {
        " OVERLAY REMOVED ".tr(is_ru)
    } else {
        " OVERLAY INSTALLED ".tr(is_ru)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().fg(colour).bg(Color::Black))
        .title(title)
        .title_alignment(Alignment::Center);

    let dim = Style::default().fg(Color::DarkGray);
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", app.overlay_install_status),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
    ];

    if let Some(path) = app.overlay_report.app_path.as_ref() {
        let text = path.display().to_string();
        let shown = if text.len() > (width as usize).saturating_sub(6) {
            format!("…{}", &text[text.len() - (width as usize - 7)..])
        } else {
            text
        };
        lines.push(Line::from(Span::styled(format!("  {shown}"), dim)));
    }

    lines.push(Line::from(Span::styled(
        if removed {
            "  Your settings are kept — [I] puts it back as it was.".tr(is_ru)
        } else {
            "  Remove it any time with [U]. Your settings stay.".tr(is_ru)
        },
        dim,
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Any key to close".tr(is_ru),
        dim,
    )));

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left),
        popup,
    );
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
        (SettingsCategory::System, "SYSTEM".tr(is_ru), "💻", "[A]"),
        (SettingsCategory::Display, "DISPLAY".tr(is_ru), "👁️", "[S]"),
        (
            SettingsCategory::RaceEngineer,
            "ENGINEER".tr(is_ru),
            "🔧",
            "[D]",
        ),
        (SettingsCategory::Overlay, "OVERLAY".tr(is_ru), "🖥️", "[F]"),
        (SettingsCategory::Keys, "KEYS".tr(is_ru), "⌨️", "[G]"),
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

            // Measured in cells, not bytes. `name.len()` counts UTF-8 bytes,
            // so ОВЕРЛЕЙ was twice as wide as this thought and the key tag ran
            // off the right edge as "[F". The icon is an emoji and takes two
            // cells; the tag is " [X] ", five; the block's borders, two.
            let used = name.chars().count() + 4 + key.chars().count() + 2;
            let spacer = Span::styled(
                " ".repeat((area.width as usize).saturating_sub(used + 2)),
                Style::default().bg(bg),
            );

            ListItem::new(Line::from(vec![name_span, spacer, key_span]))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_settings_list(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    // The other categories have five to ten items and give each a three-row
    // block. There are twenty-three bindings, which is sixty-nine rows into a
    // pane that has about thirty: every block loses its borders and the last
    // ones are squeezed to nothing. One row each, and a scroll.
    if app.ui_state.settings.category == SettingsCategory::Keys {
        render_key_settings(f, area, app);
        return;
    }

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
        SettingsCategory::Overlay => render_overlay_settings(f, &rows, app),
        // Unreachable: the key list is drawn before this function splits the
        // area, because twenty-three three-row blocks do not fit in a pane
        // that holds eleven.
        SettingsCategory::Keys => {}
    }
}

/// Every action, the key it is on, and whether the panel is waiting for a new
/// one.
///
/// Drawn from `keys::all`, which is the same list `keys::resolve` consults —
/// so a row here that says F10 is a row whose key really is F10.
///
/// One row per binding, scrolled to keep the selection on screen. There are
/// twenty-three of them and the pane holds about thirty rows, so this fits
/// today and will not when the twenty-fourth arrives; the offset is what makes
/// that a non-event.
fn render_key_settings(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let is_ru = app.config.language == Language::Russian;
    let state = &app.ui_state.settings;
    let bindings = crate::keys::all(&app.config.keys);

    let visible = area.height.max(1) as usize;
    let offset = state
        .selected_index
        .saturating_sub(visible.saturating_sub(1));

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(visible);
    for (index, (_, label, binding)) in bindings.iter().enumerate().skip(offset).take(visible) {
        let selected = index == state.selected_index;

        let value = if selected && state.capturing {
            "press a key…".tr(is_ru).to_string()
        } else {
            crate::keys::describe(binding)
        };

        // A hand-edited config with a typo in it costs one shortcut. Saying so
        // here is the difference between "that key does nothing" and "that key
        // is spelled wrong".
        let unreadable = crate::keys::parse(binding).is_none();

        let row_style = if selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let value_style = if state.capturing && selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if unreadable {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        };

        // Measured in characters, not bytes: the Russian labels are two bytes
        // each and the padding came out half as wide as it should be.
        let label_width = label.chars().count();
        let value_width = value.chars().count();
        let room = (area.width as usize).saturating_sub(label_width + value_width + 4);

        lines.push(Line::from(vec![
            Span::styled(
                format!("{} {}", if selected { "▸" } else { " " }, label),
                row_style.fg(if selected { Color::White } else { Color::Gray }),
            ),
            Span::styled(" ".repeat(room), row_style),
            Span::styled(value, value_style.bg(row_style.bg.unwrap_or(Color::Reset))),
            Span::styled("  ", row_style),
        ]));
    }

    f.render_widget(Paragraph::new(lines), area);
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
        // Whether the box is ticked is decided by *reading back the text that
        // was already drawn*, which is fragile in a way worth naming: the
        // Russian arm used to be a literal "ВКЛ" here, so translating that word
        // anywhere would have quietly emptied every checkbox on the screen.
        // Asking the catalogue for the same words the value came from keeps the
        // two ends together. The proper fix is for the caller to pass the bool
        // it already has instead of a string to be re-parsed.
        let is_on = ["ON", "SHOW"]
            .iter()
            .any(|word| value.contains(word) || value.contains(word.tr(true)));
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
                "ON".tr(is_ru)
            } else {
                "OFF".tr(is_ru)
            }
            .to_string(),
            true,
        ),
        (
            "Launcher Banner".tr(is_ru).to_string(),
            if !config.review_banner_hidden {
                "SHOW".tr(is_ru)
            } else {
                "HIDE".tr(is_ru)
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

/// The in-game panel's sections. Toggling one here changes a flag on the next
/// published frame, so the panel follows within a tick — no restart, and no
/// need to alt-tab into the game to find out whether it worked.
fn render_overlay_settings(f: &mut Frame<'_>, areas: &[Rect], app: &AppState) {
    let overlay = &app.config.overlay;
    let is_ru = app.config.language == Language::Russian;

    let items = vec![
        (
            "Telemetry section".tr(is_ru).to_string(),
            if overlay.show_telemetry { "ON" } else { "OFF" }.to_string(),
            true,
        ),
        (
            "Engineer section".tr(is_ru).to_string(),
            if overlay.show_engineer { "ON" } else { "OFF" }.to_string(),
            true,
        ),
        (
            "Session section".tr(is_ru).to_string(),
            if overlay.show_session { "ON" } else { "OFF" }.to_string(),
            true,
        ),
        (
            "Lap timing section".tr(is_ru).to_string(),
            if overlay.show_timing { "ON" } else { "OFF" }.to_string(),
            true,
        ),
        (
            "Fuel section".tr(is_ru).to_string(),
            if overlay.show_fuel { "ON" } else { "OFF" }.to_string(),
            true,
        ),
        (
            "Engineer lines".tr(is_ru).to_string(),
            overlay.engineer_lines.to_string(),
            false,
        ),
        (
            {
                // Printed from the bindings, never typed. A caption naming a
                // key that has been rebound is the exact failure
                // `the_hints_only_name_keys_that_do_something` exists to stop,
                // and these three were the last hard-coded ones left.
                let keys = &app.config.keys;
                let install = crate::keys::describe(&keys.overlay_install);
                let remove = crate::keys::describe(&keys.overlay_uninstall);
                let check = crate::keys::describe(&keys.overlay_diagnostics);
                tr_fmt(
                    "Startup card  [{0}] installs, [{1}] removes, [{2}] diagnostics",
                    is_ru,
                    &[&install, &remove, &check],
                )
            },
            if overlay.startup_card { "ON" } else { "OFF" }.to_string(),
            true,
        ),
    ];

    for (i, (label, val, is_toggle)) in items.into_iter().enumerate() {
        if i < areas.len() {
            render_item(f, areas[i], i, label, val, is_toggle, app);
        }
    }
}

fn render_engineer_settings(f: &mut Frame<'_>, areas: &[Rect], app: &AppState) {
    let alerts = &app.config.alerts;
    let config = &app.config;
    // Every value on this tab was rendered either with a hardcoded "PSI" or
    // with no unit at all, while the Display category two keys away offers
    // Bar, kPa and Fahrenheit.
    let fmt = config.formatter();
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;

    let items = vec![
        (
            tr("alert_p_min", lang),
            fmt.format_pressure(alerts.tyre_pressure_min),
            false,
        ),
        (
            tr("alert_p_max", lang),
            fmt.format_pressure(alerts.tyre_pressure_max),
            false,
        ),
        (
            tr("alert_t_min", lang),
            fmt.format_temp(alerts.tyre_temp_min),
            false,
        ),
        (
            tr("alert_t_max", lang),
            fmt.format_temp(alerts.tyre_temp_max),
            false,
        ),
        (
            tr("alert_b_max", lang),
            fmt.format_temp(alerts.brake_temp_max),
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
        (
            // Its own row, because it used to be `wear_warning - 2` and that
            // made every worn tyre a critical one two percent later.
            "Wear: critical below".tr(is_ru).to_string(),
            format!("{:.0}%", alerts.wear_critical),
            false,
        ),
        (
            "Target Hot Pressure (Front)".tr(is_ru).to_string(),
            fmt.format_pressure(config.target_hot_pressure_front),
            false,
        ),
        (
            "Target Hot Pressure (Rear)".tr(is_ru).to_string(),
            fmt.format_pressure(config.target_hot_pressure_rear),
            false,
        ),
        (
            "Ghost Delta Widget".tr(is_ru).to_string(),
            if config.show_ghost_delta {
                "ON".tr(is_ru)
            } else {
                "OFF".tr(is_ru)
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

fn render_description_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let desc = app.ui_state.settings.get_description(&app.config.language);
    let is_ru = app.config.language == Language::Russian;

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::new(2, 2, 1, 0));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // A/S/D/F/G, not A/S/D. There have been four categories since the overlay
    // one landed and five since the key map, and this line named three of them
    // -- so the two newest were reachable only by arrow key, and the help
    // overlay repeated the same wrong list.
    let controls_text =
        "[↑/↓] Select   [ENTER] Edit   [←/→] Change   [A/S/D/F/G] Categories".tr(is_ru);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    // What the last capture did, or refused to do. A binding that was declined
    // because another action already has the key looks exactly like a binding
    // that did not register, without this.
    let desc = match app.ui_state.settings.key_message.as_deref() {
        Some(message) if app.ui_state.settings.category == SettingsCategory::Keys => {
            message.to_string()
        }
        _ => desc,
    };

    let p_desc = Paragraph::new(format!("ℹ️ {}", desc)).style(Style::default().fg(Color::White));
    let p_ctrl = Paragraph::new(controls_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Right);

    f.render_widget(p_desc, chunks[0]);
    f.render_widget(p_ctrl, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    fn on_keys() -> SettingsState {
        let mut state = SettingsState::new();
        state.set_category(SettingsCategory::Keys);
        state
    }

    /// The reason `capturing` exists: the next keypress has to reach here
    /// before anything resolves it, or the overlay toggle could never be
    /// rebound — pressing it would toggle the overlay on the way past.
    #[test]
    fn enter_arms_the_capture_and_the_next_key_lands_on_the_binding() {
        let mut state = on_keys();
        let mut config = AppConfig::default();

        assert!(!state.capturing);
        state.handle_input(KeyCode::Enter, &mut config);
        assert!(state.capturing, "ENTER has to arm the capture");

        let changed = state.capture_key(
            KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE),
            &mut config,
        );

        assert!(changed, "the config changed, so the caller has to save it");
        assert!(!state.capturing, "one keypress, not a mode to escape from");
        assert_eq!(config.keys.help, "f9");
    }

    /// Two actions on one key means one of them silently stops working, with
    /// nothing on screen to say which. Refused, with a reason.
    #[test]
    fn a_key_another_action_already_has_is_refused() {
        let mut state = on_keys();
        let mut config = AppConfig::default();
        // Index 2 is the screenshot; F1 is the help.
        state.selected_index = 2;
        state.capturing = true;

        let changed = state.capture_key(
            KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
            &mut config,
        );

        assert!(!changed);
        assert_eq!(config.keys.screenshot, "ctrl+s", "left as it was");
        let message = state.key_message.expect("a reason to show the driver");
        assert!(message.contains("F1"), "{message}");
        assert!(message.contains("Help"), "{message}");
    }

    /// Escape has to leave the binding alone, or arming the capture by accident
    /// is a binding lost.
    #[test]
    fn escape_cancels_without_touching_the_binding() {
        let mut state = on_keys();
        let mut config = AppConfig::default();
        state.capturing = true;

        let changed =
            state.capture_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut config);

        assert!(!changed);
        assert!(!state.capturing);
        assert_eq!(config.keys.help, "f1");
    }

    /// A way back from a binding that turned out to be wrong, without editing
    /// the config by hand.
    #[test]
    fn delete_puts_the_default_back() {
        let mut state = on_keys();
        let mut config = AppConfig::default();
        config.keys.help = "ctrl+h".to_string();
        state.capturing = true;

        let changed = state.capture_key(
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
            &mut config,
        );

        assert!(changed);
        assert_eq!(config.keys.help, "f1");
    }

    /// Every binding has a row, and the selection cannot run off the end of
    /// the list onto one that is not drawn.
    #[test]
    fn the_key_list_has_a_row_for_every_binding() {
        let state = on_keys();
        assert_eq!(
            state.get_item_count(),
            crate::keys::all(&ac_core::config::KeyBindings::default()).len()
        );

        let mut state = on_keys();
        let mut config = AppConfig::default();
        for _ in 0..100 {
            state.handle_input(KeyCode::Down, &mut config);
        }
        assert_eq!(state.selected_index, state.get_item_count() - 1);
    }

    /// Every category has to be reachable by its advertised letter. The list
    /// said A/S/D while there were four of them, and then five.
    #[test]
    fn every_category_is_reachable_by_its_letter() {
        let mut config = AppConfig::default();
        for (key, expected) in [
            ('a', SettingsCategory::System),
            ('s', SettingsCategory::Display),
            ('d', SettingsCategory::RaceEngineer),
            ('f', SettingsCategory::Overlay),
            ('g', SettingsCategory::Keys),
        ] {
            let mut state = SettingsState::new();
            state.handle_input(KeyCode::Char(key), &mut config);
            assert_eq!(state.category, expected, "[{key}] should open {expected:?}");
        }
    }

    /// And by the arrows, all the way round in both directions.
    #[test]
    fn the_categories_cycle_both_ways() {
        let mut state = SettingsState::new();
        let start = state.category;
        for _ in 0..5 {
            state.next_category();
        }
        assert_eq!(state.category, start);
        for _ in 0..5 {
            state.prev_category();
        }
        assert_eq!(state.category, start);
    }
}
