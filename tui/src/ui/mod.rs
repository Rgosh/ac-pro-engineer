use crate::ui::localization::tr;
use crate::{AppStage, AppState, AppTab};
use ratatui::{prelude::*, widgets::*};

pub mod file_menu;
pub mod help_overlay;
pub mod launcher;
pub mod localization;
pub mod overlay;
pub mod tabs;
pub mod widgets;

pub struct UIState {
    pub theme: ac_core::config::Theme,
    pub layout_mode: LayoutMode,
    pub show_help: bool,
    pub blink_state: bool,
    pub overlay_mode: bool,
    pub last_blink: std::time::Instant,
    pub settings: tabs::settings::SettingsState,
    pub analysis: tabs::analysis::AnalysisState,
    pub engineer: tabs::engineer::EngineerState,
    pub setup_list_state: ListState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    Horizontal,
    Vertical,
    Auto,
}

impl UIState {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            theme: ac_core::config::Theme::default(),
            layout_mode: LayoutMode::Auto,
            show_help: false,
            blink_state: false,
            overlay_mode: false,
            last_blink: std::time::Instant::now(),
            settings: tabs::settings::SettingsState::new(),
            analysis: tabs::analysis::AnalysisState::new(),
            engineer: tabs::engineer::EngineerState::new(),
            setup_list_state: list_state,
        }
    }

    pub fn get_color(&self, color_tuple: &ac_core::config::ColorTuple) -> Color {
        Color::Rgb(color_tuple.r, color_tuple.g, color_tuple.b)
    }

    pub fn update_blink(&mut self) {
        if self.last_blink.elapsed() >= std::time::Duration::from_millis(500) {
            self.blink_state = !self.blink_state;
            self.last_blink = std::time::Instant::now();
        }
    }
}

pub struct UIRenderer;

impl UIRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, f: &mut Frame<'_>, app: &AppState) {
        match app.stage {
            AppStage::Launcher => launcher::render(f, f.size(), app),
            AppStage::Running => {
                if app.ui_state.overlay_mode {
                    overlay::render(f, f.size(), app);
                } else {
                    self.render_main_app(f, app);
                }

                if app.show_help {
                    let tab_idx = match app.active_tab {
                        AppTab::Dashboard => 0,
                        AppTab::Telemetry => 1,
                        AppTab::Engineer => 2,
                        AppTab::Setup => 3,
                        AppTab::Analysis => 4,
                        AppTab::Strategy => 5,
                        AppTab::Ffb => 6,
                        AppTab::Settings => 7,
                        AppTab::Guide => 8,
                    };
                    help_overlay::render(f, f.size(), tab_idx);
                }
            }
        }
    }

    fn render_main_app(&self, f: &mut Frame<'_>, app: &AppState) {
        let size = f.size();
        let is_vertical = size.height as f32 > size.width as f32 * 1.5;
        let layout_mode = if app.ui_state.layout_mode == LayoutMode::Auto {
            if is_vertical {
                LayoutMode::Vertical
            } else {
                LayoutMode::Horizontal
            }
        } else {
            app.ui_state.layout_mode
        };

        match layout_mode {
            LayoutMode::Horizontal => self.render_horizontal(f, app),
            LayoutMode::Vertical => self.render_vertical(f, app),
            LayoutMode::Auto => self.render_horizontal(f, app),
        }
    }

    fn render_horizontal(&self, f: &mut Frame<'_>, app: &AppState) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(f.size());

        self.render_header(f, main_layout[0], app);

        match app.active_tab {
            AppTab::Dashboard => tabs::dashboard::render_horizontal(f, main_layout[1], app),
            AppTab::Telemetry => tabs::telemetry::render(f, main_layout[1], app),
            AppTab::Engineer => tabs::engineer::render_horizontal(f, main_layout[1], app),
            AppTab::Setup => tabs::setup::render(f, main_layout[1], app),
            AppTab::Analysis => tabs::analysis::render(f, main_layout[1], app),
            AppTab::Strategy => tabs::strategy::render(f, main_layout[1], app),
            AppTab::Ffb => tabs::ffb::render(f, main_layout[1], app, &app.engineer),
            AppTab::Settings => tabs::settings::render(f, main_layout[1], app),
            AppTab::Guide => tabs::guide::render(f, main_layout[1], app),
        }

        self.render_footer(f, main_layout[2], app);
    }

    fn render_vertical(&self, f: &mut Frame<'_>, app: &AppState) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(12),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(f.size());

        self.render_header(f, main_layout[0], app);
        widgets::render_telemetry_bar_vertical(f, main_layout[1], app);

        match app.active_tab {
            AppTab::Dashboard => tabs::dashboard::render_vertical(f, main_layout[2], app),
            AppTab::Telemetry => tabs::telemetry::render(f, main_layout[2], app),
            AppTab::Engineer => tabs::engineer::render_vertical(f, main_layout[2], app),
            AppTab::Setup => tabs::setup::render(f, main_layout[2], app),
            AppTab::Analysis => tabs::analysis::render(f, main_layout[2], app),
            AppTab::Strategy => tabs::strategy::render(f, main_layout[2], app),
            AppTab::Ffb => tabs::ffb::render(f, main_layout[2], app, &app.engineer),
            AppTab::Settings => tabs::settings::render(f, main_layout[2], app),
            AppTab::Guide => tabs::guide::render(f, main_layout[2], app),
        }

        self.render_footer(f, main_layout[3], app);
    }

    fn render_header(&self, f: &mut Frame<'_>, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        let lang = &app.config.language;

        let mut rpm_ratio = 0.0;
        let mut current_rpm: i32 = 0;
        let mut max_rpm: i32 = 8000;

        if let Some(phys) = app.physics_history.last() {
            current_rpm = phys.rpms as i32;
            let game_max = app.session_info.max_rpm;

            if game_max > 0 {
                max_rpm = game_max;
            }
            if current_rpm > max_rpm {
                max_rpm = current_rpm;
            }
            if max_rpm > 0 {
                rpm_ratio = (current_rpm as f32 / max_rpm as f32).clamp(0.0, 1.0);
            }
        }

        let header_style = if rpm_ratio > 0.96 {
            if app.ui_state.blink_state {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            }
        } else {
            Style::default().fg(app.ui_state.get_color(&theme.text))
        };

        if rpm_ratio > 0.96 {
            let flash_block = Block::default().style(header_style);
            f.render_widget(flash_block, area);
        }

        let tabs = vec![
            format!("🏁 {}", tr("tab_dash", lang)),
            format!("📊 {}", tr("tab_tele", lang)),
            format!("👨‍🔧 {}", tr("tab_eng", lang)),
            format!("🔧 {}", tr("tab_setup", lang)),
            format!("📈 {}", tr("tab_anal", lang)),
            format!("🎯 {}", tr("tab_strat", lang)),
            "🎮 FFB".to_string(),
            format!("⚙️ {}", tr("tab_set", lang)),
            "📖 Guide".to_string(),
        ];

        let active_index = match app.active_tab {
            AppTab::Dashboard => 0,
            AppTab::Telemetry => 1,
            AppTab::Engineer => 2,
            AppTab::Setup => 3,
            AppTab::Analysis => 4,
            AppTab::Strategy => 5,
            AppTab::Ffb => 6,
            AppTab::Settings => 7,
            AppTab::Guide => 8,
        };

        let tab_widget = Tabs::new(tabs)
            .select(active_index)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
            )
            .style(header_style)
            .highlight_style(
                Style::default()
                    .fg(app.ui_state.get_color(&theme.highlight))
                    .add_modifier(Modifier::BOLD),
            )
            .divider("│");

        f.render_widget(tab_widget, area);

        if max_rpm > 0 {
            let gauge_area = Rect {
                x: area.x,
                y: area.y + area.height.saturating_sub(1),
                width: area.width,
                height: 1,
            };

            if rpm_ratio > 0.0 {
                let gauge_width = (area.width as f32 * rpm_ratio) as u16;
                let gauge_color = if rpm_ratio > 0.96 {
                    if app.ui_state.blink_state {
                        Color::Red
                    } else {
                        Color::White
                    }
                } else if rpm_ratio > 0.9 {
                    Color::Red
                } else if rpm_ratio > 0.75 {
                    Color::Yellow
                } else {
                    Color::Green
                };

                let bar_area = Rect {
                    width: gauge_width,
                    ..gauge_area
                };

                let gauge_block = Block::default().style(Style::default().bg(gauge_color));
                f.render_widget(gauge_block, bar_area);
            }

            let rpm_text = format!("{} / {} RPM", current_rpm, max_rpm);
            let text_widget = Paragraph::new(rpm_text).alignment(Alignment::Center).style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );

            f.render_widget(text_widget, gauge_area);
        }
    }

    fn render_footer(&self, f: &mut Frame<'_>, area: Rect, app: &AppState) {
        let (air, road, fuel, last, best) = if let Some(phys) = app.physics_history.last() {
            let gfx = app.graphics_history.last();
            let l = gfx.map(|g| g.i_last_time).unwrap_or(0);
            let b = gfx.map(|g| g.i_best_time).unwrap_or(0);
            (phys.air_temp, phys.road_temp, phys.fuel, l, b)
        } else {
            (0.0, 0.0, 0.0, 0, 0)
        };

        let car = if app.session_info.car_name.is_empty() {
            "No Car".to_string()
        } else {
            app.session_info.car_name.clone()
        };
        let track = if app.session_info.track_name.is_empty() {
            "No Track".to_string()
        } else {
            app.session_info.track_name.clone()
        };

        let fmt_lap = |ms: i32| -> String {
            if ms <= 0 {
                return "-:--.---".to_string();
            };
            let m = ms / 60000;
            let s = (ms % 60000) / 1000;
            let mil = ms % 1000;
            format!("{}:{:02}.{:03}", m, s, mil)
        };

        let status_text = if app.is_connected {
            " ONLINE "
        } else {
            " OFFLINE "
        };
        let status_style = if app.is_connected {
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        };

        let spans = vec![
            Span::styled(status_text, status_style),
            Span::raw(" "),
            Span::styled(
                format!(" 🏎️ {} ", car),
                Style::default().bg(Color::Blue).fg(Color::Black),
            ),
            Span::raw(" "),
            Span::styled(
                format!(" 🗺️ {} ", track),
                Style::default().bg(Color::Cyan).fg(Color::Black),
            ),
            Span::raw(" "),
            Span::styled(
                format!(" ⛽ {:.1} L ", fuel),
                Style::default().bg(Color::Red).fg(Color::White),
            ),
            Span::raw(" "),
            Span::styled(
                format!(" L: {} ", fmt_lap(last)),
                Style::default().bg(Color::DarkGray).fg(Color::White),
            ),
            Span::raw(" "),
            Span::styled(
                format!(" B: {} ", fmt_lap(best)),
                Style::default().bg(Color::Magenta).fg(Color::White),
            ),
            Span::raw(" "),
            Span::styled(
                format!(" 🌡️ A:{:.0}° R:{:.0}° ", air, road),
                Style::default().bg(Color::Yellow).fg(Color::Black),
            ),
            Span::raw(" "),
            Span::styled(
                " [F10: Mini] [H: Help] ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];

        let footer = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Left)
            .style(Style::default().bg(Color::Reset));

        f.render_widget(footer, area);
    }
}
