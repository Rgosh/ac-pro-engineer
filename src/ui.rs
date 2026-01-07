use ratatui::{prelude::*, widgets::*, text::Line};
use crate::{AppState, AppTab};

pub struct UIState {
    pub theme: crate::config::Theme,
    pub layout_mode: LayoutMode,
    pub show_help: bool,
    pub blink_state: bool,
    pub last_blink: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    Horizontal,
    Vertical,
    Auto,
}

impl UIState {
    pub fn new(theme: &crate::config::Theme) -> Self {
        Self {
            theme: theme.clone(),
            layout_mode: LayoutMode::Auto,
            show_help: false,
            blink_state: false,
            last_blink: std::time::Instant::now(),
        }
    }
    
    pub fn get_color(&self, color_tuple: &crate::config::ColorTuple) -> Color {
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
    
    pub fn render(&self, f: &mut Frame, app: &AppState) {
        // Blink update moved to AppState::tick()
        
        let size = f.size();
        let is_vertical = size.height as f32 > size.width as f32 * 1.5;
        let layout_mode = if app.ui_state.layout_mode == LayoutMode::Auto {
            if is_vertical { LayoutMode::Vertical } else { LayoutMode::Horizontal }
        } else {
            app.ui_state.layout_mode
        };
        
        match layout_mode {
            LayoutMode::Horizontal => self.render_horizontal(f, app),
            LayoutMode::Vertical => self.render_vertical(f, app),
            LayoutMode::Auto => self.render_horizontal(f, app),
        }
    }
    
    fn render_horizontal(&self, f: &mut Frame, app: &AppState) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(f.size());
        
        self.render_header(f, main_layout[0], app);
        self.render_main_horizontal(f, main_layout[1], app);
        self.render_footer(f, main_layout[2], app);
    }
    
    fn render_vertical(&self, f: &mut Frame, app: &AppState) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(12),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(f.size());
        
        self.render_header(f, main_layout[0], app);
        self.render_telemetry_bar_vertical(f, main_layout[1], app);
        
        match app.active_tab {
            AppTab::Dashboard => self.render_dashboard_vertical(f, main_layout[2], app),
            AppTab::Telemetry => self.render_telemetry_vertical(f, main_layout[2], app),
            AppTab::Engineer => self.render_engineer_vertical(f, main_layout[2], app),
            AppTab::Setup => self.render_setup_vertical(f, main_layout[2], app),
            AppTab::Analysis => self.render_analysis_vertical(f, main_layout[2], app),
            AppTab::Strategy => self.render_strategy_vertical(f, main_layout[2], app),
            AppTab::Settings => self.render_settings_vertical(f, main_layout[2], app),
        }
        
        self.render_footer(f, main_layout[3], app);
    }
    
    fn render_header(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        
        let tabs = vec![
            "🏁 DASHBOARD",
            "📊 TELEMETRY", 
            "👨‍🔧 ENGINEER",
            "🔧 SETUP",
            "📈 ANALYSIS",
            "🎯 STRATEGY",
            "⚙️ SETTINGS",
        ];
        
        let tab_widget = Tabs::new(tabs)
            .select(match app.active_tab {
                AppTab::Dashboard => 0,
                AppTab::Telemetry => 1,
                AppTab::Engineer => 2,
                AppTab::Setup => 3,
                AppTab::Analysis => 4,
                AppTab::Strategy => 5,
                AppTab::Settings => 6,
            })
            .block(Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))))
            .style(Style::default().fg(app.ui_state.get_color(&theme.text)))
            .highlight_style(Style::default()
                .fg(app.ui_state.get_color(&theme.highlight))
                .add_modifier(Modifier::BOLD))
            .divider("│");
        
        f.render_widget(tab_widget, area);
    }
    
    fn render_main_horizontal(&self, f: &mut Frame, area: Rect, app: &AppState) {
        match app.active_tab {
            AppTab::Dashboard => self.render_dashboard_horizontal(f, area, app),
            AppTab::Telemetry => self.render_telemetry_horizontal(f, area, app),
            AppTab::Engineer => self.render_engineer_horizontal(f, area, app),
            AppTab::Setup => self.render_setup_horizontal(f, area, app),
            AppTab::Analysis => self.render_analysis_horizontal(f, area, app),
            AppTab::Strategy => self.render_strategy_horizontal(f, area, app),
            AppTab::Settings => self.render_settings_horizontal(f, area, app),
        }
    }
    
    fn render_dashboard_horizontal(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(area);
        
        self.render_tyre_panel(f, layout[0], app);
        self.render_central_gauges(f, layout[1], app);
        self.render_info_panel(f, layout[2], app);
    }
    
    fn render_tyre_panel(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        let block = Block::default()
            .title("TYRE STATUS")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
        let inner = block.inner(area);
        let tyre_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Min(0),
            ])
            .split(inner);
        
        let front_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(tyre_layout[0]);
        
        self.render_tyre_widget(f, front_layout[0], 0, app, "FL");
        self.render_tyre_widget(f, front_layout[1], 1, app, "FR");
        
        let rear_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(tyre_layout[1]);
        
        self.render_tyre_widget(f, rear_layout[0], 2, app, "RL");
        self.render_tyre_widget(f, rear_layout[1], 3, app, "RR");
        
        if let Some(phys) = &app.physics_mem {
            let data = phys.get();
            let avg_pressure: f32 = data.wheels_pressure.iter().sum::<f32>() / 4.0;
            let avg_temp: f32 = (0..4).map(|i| data.get_avg_tyre_temp(i)).sum::<f32>() / 4.0;
            let avg_wear: f32 = data.tyre_wear.iter().sum::<f32>() / 4.0;
            
            let summary = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("Avg Pressure: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                    Span::styled(format!("{:.1} psi", avg_pressure), 
                        Style::default().fg(self.get_pressure_color(avg_pressure))),
                ]),
                Line::from(vec![
                    Span::styled("Avg Temp: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                    Span::styled(format!("{:.0}°C", avg_temp),
                        Style::default().fg(self.get_tyre_color(avg_temp))),
                ]),
                Line::from(vec![
                    Span::styled("Avg Wear: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                    Span::styled(format!("{:.1}%", avg_wear),
                        Style::default().fg(self.get_wear_color(avg_wear))),
                ]),
            ]).block(Block::default());
            
            f.render_widget(summary, tyre_layout[2]);
        }
        
        f.render_widget(block, area);
    }
    
    fn render_central_gauges(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        let block = Block::default()
            .title("PERFORMANCE")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
        let inner = block.inner(area);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(6),
                Constraint::Length(4),
                Constraint::Min(0),
            ])
            .split(inner);
        
        if let Some(phys) = &app.physics_mem {
            let data = phys.get();
            let gear = match data.gear {
                0 => "R".to_string(),
                1 => "N".to_string(),
                n => format!("{}", n - 1),
            };
            
            let speed_gear = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(format!("{:3}", data.speed_kmh as i32), 
                        Style::default()
                            .fg(app.ui_state.get_color(&theme.highlight))
                            .add_modifier(Modifier::BOLD)
                            .add_modifier(Modifier::ITALIC)),
                    Span::raw(" km/h  "),
                    Span::styled(gear, 
                        Style::default()
                            .fg(app.ui_state.get_color(&theme.accent))
                            .add_modifier(Modifier::BOLD)),
                ]).alignment(Alignment::Center),
            ]);
            
            f.render_widget(speed_gear, layout[0]);
        }
        
        if let Some(phys) = &app.physics_mem {
            let data = phys.get();
            // ИСПРАВЛЕНИЕ ЗДЕСЬ: добавляем .clamp(0.0, 1.0)
            // Также защищаемся от деления на ноль, если max_rpm еще не загрузился
            let max_rpm = if app.session_info.max_rpm > 0 { app.session_info.max_rpm as f32 } else { 8000.0 };
            let rpm_percent = (data.rpms as f32 / max_rpm).clamp(0.0, 1.0);
            
            let rpm_gauge = Gauge::default()
                .block(Block::default().title("RPM"))
                .gauge_style(Style::default()
                    .fg(self.get_rpm_color(rpm_percent))
                    .bg(Color::DarkGray))
                .ratio(rpm_percent as f64)
                .label(format!("{:5}", data.rpms));
            
            f.render_widget(rpm_gauge, layout[1]);
        }
        
        if let Some(phys) = &app.physics_mem {
            let data = phys.get();
            let pedal_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Length(3)])
                .split(layout[2]);
            
            // ИСПРАВЛЕНИЕ ЗДЕСЬ: добавляем .clamp(0.0, 1.0) для педалей
            let throttle = Gauge::default()
                .block(Block::default().title("Throttle"))
                .gauge_style(Style::default().fg(Color::Green))
                .ratio((data.gas as f64).clamp(0.0, 1.0))
                .label(format!("{:.0}%", data.gas * 100.0));
            
            let brake = Gauge::default()
                .block(Block::default().title("Brake"))
                .gauge_style(Style::default().fg(Color::Red))
                .ratio((data.brake as f64).clamp(0.0, 1.0))
                .label(format!("{:.0}%", data.brake * 100.0));
            
            f.render_widget(throttle, pedal_layout[0]);
            f.render_widget(brake, pedal_layout[1]);
        }
        
        let delta = app.engineer.stats.current_delta;
        let delta_sign = if delta >= 0.0 { "+" } else { "" };
        let delta_color = self.get_delta_color(delta);
        
        // Исправляем доступ к blink_state, так как мы его перенесли в AppState
        // (если вы уже применили исправления из предыдущего ответа про blink_state, 
        // то app.ui_state.blink_state здесь корректен, так как render берет &AppState)
        let delta_blink = if delta > 1.0 && app.ui_state.blink_state {
            Modifier::SLOW_BLINK
        } else {
            Modifier::empty()
        };
        
        let delta_widget = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("DELTA: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(
                    format!("{}{:.3}", delta_sign, delta),
                    Style::default()
                        .fg(delta_color)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(delta_blink),
                ),
            ]).alignment(Alignment::Center),
        ]);
        
        f.render_widget(delta_widget, layout[3]);
        
        if let Some(gfx) = &app.graphics_mem {
            let data = gfx.get();
            let electronics = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("TC: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                    Span::styled(format!("{}", data.tc), Style::default().fg(Color::Yellow)),
                    Span::raw("  "),
                    Span::styled("ABS: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                    Span::styled(format!("{}", data.abs), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::styled("Engine Map: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                    Span::styled(format!("{}", data.engine_map), Style::default().fg(Color::Magenta)),
                ]),
            ]);
            
            f.render_widget(electronics, layout[4]);
        }
        
        f.render_widget(block, area);
    }
    
    fn render_info_panel(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        let block = Block::default()
            .title("SESSION INFO")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
        let inner = block.inner(area);
        
        let info_lines = vec![
            Line::from(vec![
                Span::styled("Car: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(&app.session_info.car_name, Style::default().fg(app.ui_state.get_color(&theme.highlight))),
            ]),
            Line::from(vec![
                Span::styled("Track: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(&app.session_info.track_name, Style::default().fg(app.ui_state.get_color(&theme.highlight))),
            ]),
            Line::from(vec![
                Span::styled("Session: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(&app.session_info.session_type, Style::default().fg(app.ui_state.get_color(&theme.accent))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Laps: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(format!("{}", app.session_info.lap_count), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Position: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(
                    if let Some(gfx) = &app.graphics_mem {
                        format!("{}", gfx.get().position)
                    } else {
                        "-".to_string()
                    },
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Fuel: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(
                    if let Some(phys) = &app.physics_mem {
                        format!("{:.1}L ({:.1} laps)", phys.get().fuel, app.engineer.stats.fuel_laps_remaining)
                    } else {
                        "-".to_string()
                    },
                    Style::default().fg(self.get_fuel_color(app.engineer.stats.fuel_laps_remaining)),
                ),
            ]),
        ];
        
        let info_widget = Paragraph::new(info_lines)
            .block(Block::default());
        
        f.render_widget(info_widget, inner);
        f.render_widget(block, area);
    }
    
    fn render_telemetry_bar_vertical(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Min(0),
            ])
            .split(area);
        
        if let Some(phys) = &app.physics_mem {
            let data = phys.get();
            
            let speed_block = Block::default()
                .title("SPEED")
                .borders(Borders::ALL);
            let speed = Paragraph::new(format!("{}\nkm/h", data.speed_kmh as i32))
                .style(Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center)
                .block(speed_block);
            f.render_widget(speed, layout[0]);
            
            let rpm_block = Block::default()
                .title("RPM")
                .borders(Borders::ALL);
            let rpm = Paragraph::new(format!("{}\nRPM", data.rpms))
                .style(Style::default()
                    .fg(self.get_rpm_color(data.rpms as f32 / app.session_info.max_rpm as f32))
                    .add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center)
                .block(rpm_block);
            f.render_widget(rpm, layout[1]);
            
            let gear = match data.gear {
                0 => "R",
                1 => "N",
                n => &format!("{}", n - 1),
            };
            let gear_block = Block::default()
                .title("GEAR")
                .borders(Borders::ALL);
            let gear_widget = Paragraph::new(gear)
                .style(Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center)
                .block(gear_block);
            f.render_widget(gear_widget, layout[2]);
            
            let delta = app.engineer.stats.current_delta;
            let delta_sign = if delta >= 0.0 { "+" } else { "" };
            let delta_block = Block::default()
                .title("DELTA")
                .borders(Borders::ALL);
            let delta_widget = Paragraph::new(format!("{}{:.3}", delta_sign, delta))
                .style(Style::default()
                    .fg(self.get_delta_color(delta))
                    .add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center)
                .block(delta_block);
            f.render_widget(delta_widget, layout[3]);
        }
    }
    
    fn render_engineer_horizontal(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 2),
                Constraint::Ratio(1, 2),
            ])
            .split(area);
        
        self.render_recommendations(f, layout[0], app);
        self.render_analysis(f, layout[1], app);
    }
    
    fn render_recommendations(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        let block = Block::default()
            .title("RECOMMENDATIONS")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
        if app.recommendations.is_empty() {
            let message = Paragraph::new(vec![
                Line::from(""),
                Line::from("All systems operating within optimal parameters."),
                Line::from(""),
                Line::from("Keep pushing! 🏎️💨"),
            ])
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Center);
            
            f.render_widget(message, area);
        } else {
            let items: Vec<ListItem> = app.recommendations.iter().take(8).map(|rec| {
                let severity_color = match rec.severity {
                    crate::engineer::Severity::Critical => Color::Red,
                    crate::engineer::Severity::Warning => Color::Yellow,
                    crate::engineer::Severity::Info => Color::Blue,
                };
                
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("● ", Style::default().fg(severity_color)),
                        Span::styled(&rec.component, Style::default().fg(app.ui_state.get_color(&theme.highlight))),
                        Span::raw(" - "),
                        Span::styled(&rec.message, Style::default().fg(app.ui_state.get_color(&theme.text))),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled("Action: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(&rec.action, Style::default().fg(app.ui_state.get_color(&theme.accent))),
                    ]),
                ];
                
                for param in &rec.parameters {
                    lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::styled(format!("{}: ", param.name), Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{:.1}{}", param.current, param.unit), Style::default().fg(Color::White)),
                        Span::raw(" → "),
                        Span::styled(format!("{:.1}{}", param.target, param.unit), Style::default().fg(Color::Green)),
                    ]));
                }
                
                lines.push(Line::from(""));
                ListItem::new(lines)
            }).collect();
            
            let list = List::new(items)
                .block(block)
                .style(Style::default().fg(app.ui_state.get_color(&theme.text)));
            
            f.render_widget(list, area);
        }
    }
    
    fn render_analysis(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        let block = Block::default()
            .title("DRIVING ANALYSIS")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
        let inner = block.inner(area);
        
        let analysis = vec![
            Line::from(vec![
                Span::styled("Smoothness: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                self.render_progress_bar(app.engineer.driving_style.smoothness, 100.0),
            ]),
            Line::from(vec![
                Span::styled("Aggression: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                self.render_progress_bar(app.engineer.driving_style.aggression * 100.0, 100.0),
            ]),
            Line::from(vec![
                Span::styled("Trail Braking: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                self.render_progress_bar(app.engineer.driving_style.trail_braking * 100.0, 100.0),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Lockups: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(
                    format!("{}", app.engineer.stats.lockup_frames),
                    Style::default().fg(if app.engineer.stats.lockup_frames > 10 { Color::Red } else { Color::Green }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Wheel Spin: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(
                    format!("{}", app.engineer.stats.wheel_spin_frames),
                    Style::default().fg(if app.engineer.stats.wheel_spin_frames > 15 { Color::Yellow } else { Color::Green }),
                ),
            ]),
        ];
        
        let analysis_widget = Paragraph::new(analysis)
            .block(Block::default());
        
        f.render_widget(analysis_widget, inner);
        f.render_widget(block, area);
    }
    
    fn render_progress_bar(&self, value: f32, max: f32) -> Span {
        let percent = (value / max * 100.0).min(100.0);
        let filled = (percent / 10.0).floor() as usize;
        let bar = "█".repeat(filled) + &"░".repeat(10 - filled);
        
        let color = if percent < 30.0 {
            Color::Red
        } else if percent < 70.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        
        Span::styled(format!(" {:3.0}% {}", percent, bar), Style::default().fg(color))
    }
    
    fn render_tyre_widget(&self, f: &mut Frame, area: Rect, index: usize, app: &AppState, label: &str) {
        if let Some(phys) = &app.physics_mem {
            let data = phys.get();
            
            let theme = &app.ui_state.theme;
            let block = Block::default()
                .title(label)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
            
            let inner = block.inner(area);
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .split(inner);
            
            let pressure = data.wheels_pressure[index];
            let pressure_text = format!("{:.1} psi", pressure);
            let pressure_widget = Paragraph::new(pressure_text)
                .style(Style::default().fg(self.get_pressure_color(pressure)))
                .alignment(Alignment::Center);
            
            let temp_i = data.tyre_temp_i[index];
            let temp_m = data.tyre_temp_m[index];
            let temp_o = data.tyre_temp_o[index];
            let avg_temp = (temp_i + temp_m + temp_o) / 3.0;
            let temp_text = format!("I{:.0} M{:.0} O{:.0}", temp_i, temp_m, temp_o);
            let temp_widget = Paragraph::new(temp_text)
                .style(Style::default().fg(self.get_tyre_color(avg_temp)))
                .alignment(Alignment::Center);
            
            let wear = data.tyre_wear[index];
            let wear_text = format!("{:.1}%", wear);
            let wear_widget = Paragraph::new(wear_text)
                .style(Style::default().fg(self.get_wear_color(wear)))
                .alignment(Alignment::Center);
            
            let brake_temp = data.brake_temp[index];
            let brake_text = format!("B{:.0}°C", brake_temp);
            let brake_widget = Paragraph::new(brake_text)
                .style(Style::default().fg(self.get_brake_color(brake_temp)))
                .alignment(Alignment::Center);
            
            f.render_widget(pressure_widget, layout[0]);
            f.render_widget(temp_widget, layout[1]);
            f.render_widget(wear_widget, layout[2]);
            f.render_widget(brake_widget, layout[3]);
            f.render_widget(block, area);
        }
    }
    
    fn get_tyre_color(&self, temp: f32) -> Color {
        match temp {
            t if t < 70.0 => Color::Blue,
            t if t < 85.0 => Color::Cyan,
            t if t < 95.0 => Color::Green,
            t if t < 105.0 => Color::Yellow,
            _ => Color::Red,
        }
    }
    
    fn get_pressure_color(&self, psi: f32) -> Color {
        match psi {
            p if p < 26.0 => Color::Blue,
            p if p <= 27.5 => Color::Green,
            p if p <= 28.5 => Color::Yellow,
            _ => Color::Red,
        }
    }
    
    fn get_brake_color(&self, temp: f32) -> Color {
        match temp {
            t if t < 300.0 => Color::Blue,
            t if t < 500.0 => Color::Green,
            t if t < 700.0 => Color::Yellow,
            _ => Color::Red,
        }
    }
    
    fn get_wear_color(&self, wear: f32) -> Color {
        match wear {
            w if w < 30.0 => Color::Green,
            w if w < 60.0 => Color::Yellow,
            w if w < 80.0 => Color::LightRed,
            _ => Color::Red,
        }
    }
    
    fn get_rpm_color(&self, rpm_percent: f32) -> Color {
        match rpm_percent {
            r if r < 0.7 => Color::Green,
            r if r < 0.85 => Color::Yellow,
            r if r < 0.95 => Color::LightRed,
            _ => Color::Red,
        }
    }
    
    fn get_delta_color(&self, delta: f32) -> Color {
        match delta {
            d if d < -0.5 => Color::Magenta,
            d if d < -0.1 => Color::Green,
            d if d < 0.1 => Color::Yellow,
            d if d < 0.5 => Color::LightRed,
            _ => Color::Red,
        }
    }
    
    fn get_fuel_color(&self, laps_remaining: f32) -> Color {
        match laps_remaining {
            l if l > 5.0 => Color::Green,
            l if l > 2.0 => Color::Yellow,
            l if l > 0.5 => Color::LightRed,
            _ => Color::Red,
        }
    }
    
    fn render_telemetry_horizontal(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Telemetry view - Detailed graphs and data")
            .block(Block::default().title("TELEMETRY").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_setup_horizontal(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Setup view - Compare and adjust car setup")
            .block(Block::default().title("SETUP").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_analysis_horizontal(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Analysis view - Lap analysis and comparisons")
            .block(Block::default().title("ANALYSIS").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_strategy_horizontal(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Strategy view - Race strategy and pit stops")
            .block(Block::default().title("STRATEGY").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_settings_horizontal(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Settings view - Configure application")
            .block(Block::default().title("SETTINGS").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_dashboard_vertical(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(12),
                Constraint::Min(0),
            ])
            .split(area);
        
        self.render_tyres_vertical(f, layout[0], app);
        self.render_quick_info_vertical(f, layout[1], app);
    }
    
    fn render_tyres_vertical(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(area);
        
        let left_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(6),
            ])
            .split(layout[0]);
        
        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(6),
            ])
            .split(layout[1]);
        
        self.render_tyre_widget(f, left_layout[0], 0, app, "FL");
        self.render_tyre_widget(f, left_layout[1], 2, app, "RL");
        self.render_tyre_widget(f, right_layout[0], 1, app, "FR");
        self.render_tyre_widget(f, right_layout[1], 3, app, "RR");
    }
    
    fn render_quick_info_vertical(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        let info = vec![
            Line::from(vec![
                Span::styled("Fuel: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(
                    format!("{:.1}L", 
                        app.physics_mem.as_ref().map_or(0.0, |p| p.get().fuel)),
                    Style::default().fg(self.get_fuel_color(app.engineer.stats.fuel_laps_remaining)),
                ),
                Span::raw(" ("),
                Span::styled(
                    format!("{:.1} laps", app.engineer.stats.fuel_laps_remaining),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(")"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("TC: ", Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(
                    app.graphics_mem.as_ref().map_or("-".to_string(), |g| g.get().tc.to_string()),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw("  ABS: "),
                Span::styled(
                    app.graphics_mem.as_ref().map_or("-".to_string(), |g| g.get().abs.to_string()),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
        ];
        
        let info_widget = Paragraph::new(info)
            .block(Block::default()
                .title("QUICK INFO")
                .borders(Borders::ALL))
            .alignment(Alignment::Left);
        
        f.render_widget(info_widget, area);
    }
    
    fn render_footer(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let status = if app.is_connected {
            let blink = if app.ui_state.blink_state { "●" } else { "○" };
            format!("{} CONNECTED | F1-F7: Tabs | Tab: Cycle | Q: Quit", blink)
        } else {
            "DISCONNECTED - Waiting for Assetto Corsa...".to_string()
        };
        
        let status_color = if app.is_connected { Color::Green } else { Color::Red };
        
        let footer = Paragraph::new(status)
            .style(Style::default().fg(status_color))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::TOP));
        
        f.render_widget(footer, area);
    }
    
    fn render_telemetry_vertical(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Vertical Telemetry View")
            .block(Block::default().title("TELEMETRY").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_engineer_vertical(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Vertical Engineer View")
            .block(Block::default().title("ENGINEER").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_setup_vertical(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Vertical Setup View")
            .block(Block::default().title("SETUP").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_analysis_vertical(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Vertical Analysis View")
            .block(Block::default().title("ANALYSIS").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_strategy_vertical(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Vertical Strategy View")
            .block(Block::default().title("STRATEGY").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
    
    fn render_settings_vertical(&self, f: &mut Frame, area: Rect, _app: &AppState) {
        let text = Paragraph::new("Vertical Settings View")
            .block(Block::default().title("SETTINGS").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
    }
}