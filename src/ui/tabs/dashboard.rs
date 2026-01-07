use ratatui::{prelude::*, widgets::*};
use crate::AppState;
use crate::ui::widgets::*;
use crate::ui::localization::tr;

pub fn render_horizontal(f: &mut Frame, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);
    
    render_tyre_panel(f, layout[0], app);
    render_central_gauges(f, layout[1], app);
    render_info_panel(f, layout[2], app);
}

pub fn render_vertical(f: &mut Frame, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),
            Constraint::Min(0),
        ])
        .split(area);
    
    render_tyres_vertical(f, layout[0], app);
    render_quick_info_vertical(f, layout[1], app);
}

fn render_tyre_panel(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("dash_tyre_status", lang))
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
    
    render_tyre_widget(f, front_layout[0], 0, app, "FL");
    render_tyre_widget(f, front_layout[1], 1, app, "FR");
    
    let rear_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(tyre_layout[1]);
    
    render_tyre_widget(f, rear_layout[0], 2, app, "RL");
    render_tyre_widget(f, rear_layout[1], 3, app, "RR");
    
    if let Some(phys) = &app.physics_mem {
        let data = phys.get();
        let avg_pressure: f32 = data.wheels_pressure.iter().sum::<f32>() / 4.0;
        let avg_temp: f32 = (0..4).map(|i| data.get_avg_tyre_temp(i)).sum::<f32>() / 4.0;
        let avg_wear: f32 = data.tyre_wear.iter().sum::<f32>() / 4.0;
        
        let summary = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!("{}: ", tr("avg_press", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(format!("{:.1} psi", avg_pressure), 
                    Style::default().fg(get_pressure_color(avg_pressure))),
            ]),
            Line::from(vec![
                Span::styled(format!("{}: ", tr("avg_temp", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(format!("{:.0}°C", avg_temp),
                    Style::default().fg(get_tyre_color(avg_temp))),
            ]),
            Line::from(vec![
                Span::styled(format!("{}: ", tr("avg_wear", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(format!("{:.1}%", avg_wear),
                    Style::default().fg(get_wear_color(avg_wear))),
            ]),
        ]).block(Block::default());
        
        f.render_widget(summary, tyre_layout[2]);
    }
    
    f.render_widget(block, area);
}

fn render_central_gauges(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("dash_perf", lang))
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
        let max_rpm = if app.session_info.max_rpm > 0 { app.session_info.max_rpm as f32 } else { 8000.0 };
        let rpm_percent = (data.rpms as f32 / max_rpm).clamp(0.0, 1.0);
        
        let rpm_gauge = Gauge::default()
            .block(Block::default().title(tr("lbl_rpm", lang)))
            .gauge_style(Style::default()
                .fg(get_rpm_color(rpm_percent))
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
        
        let throttle = Gauge::default()
            .block(Block::default().title(tr("lbl_throttle", lang)))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio((data.gas as f64).clamp(0.0, 1.0))
            .label(format!("{:.0}%", data.gas * 100.0));
        
        let brake = Gauge::default()
            .block(Block::default().title(tr("lbl_brake", lang)))
            .gauge_style(Style::default().fg(Color::Red))
            .ratio((data.brake as f64).clamp(0.0, 1.0))
            .label(format!("{:.0}%", data.brake * 100.0));
        
        f.render_widget(throttle, pedal_layout[0]);
        f.render_widget(brake, pedal_layout[1]);
    }
    
    let delta = app.engineer.stats.current_delta;
    let delta_sign = if delta >= 0.0 { "+" } else { "" };
    let delta_color = get_delta_color(delta);
    
    let delta_blink = if delta > 1.0 && app.ui_state.blink_state {
        Modifier::SLOW_BLINK
    } else {
        Modifier::empty()
    };
    
    let delta_widget = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(format!("{}: ", tr("lbl_delta", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
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
                Span::styled(format!("{}: ", tr("lbl_tc", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(format!("{}", data.tc), Style::default().fg(Color::Yellow)),
                Span::raw("  "),
                Span::styled(format!("{}: ", tr("lbl_abs", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(format!("{}", data.abs), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled(format!("{}: ", tr("lbl_map", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
                Span::styled(format!("{}", data.engine_map), Style::default().fg(Color::Magenta)),
            ]),
        ]);
        
        f.render_widget(electronics, layout[4]);
    }
    
    f.render_widget(block, area);
}

fn render_info_panel(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("dash_session", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    
    let inner = block.inner(area);
    
    let info_lines = vec![
        Line::from(vec![
            Span::styled(format!("{}: ", tr("lbl_car", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
            Span::styled(&app.session_info.car_name, Style::default().fg(app.ui_state.get_color(&theme.highlight))),
        ]),
        Line::from(vec![
            Span::styled(format!("{}: ", tr("lbl_track", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
            Span::styled(&app.session_info.track_name, Style::default().fg(app.ui_state.get_color(&theme.highlight))),
        ]),
        Line::from(vec![
            Span::styled(format!("{}: ", tr("lbl_session", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
            Span::styled(&app.session_info.session_type, Style::default().fg(app.ui_state.get_color(&theme.accent))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{}: ", tr("lbl_laps", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
            Span::styled(format!("{}", app.session_info.lap_count), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(format!("{}: ", tr("lbl_pos", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
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
            Span::styled(format!("{}: ", tr("lbl_fuel", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
            Span::styled(
                if let Some(phys) = &app.physics_mem {
                    format!("{:.1}L ({:.1} {})", phys.get().fuel, app.engineer.stats.fuel_laps_remaining, if app.config.language == crate::config::Language::Russian { "кр" } else { "laps" })
                } else {
                    "-".to_string()
                },
                Style::default().fg(get_fuel_color(app.engineer.stats.fuel_laps_remaining)),
            ),
        ]),
    ];
    
    let info_widget = Paragraph::new(info_lines)
        .block(Block::default());
    
    f.render_widget(info_widget, inner);
    f.render_widget(block, area);
}

fn render_tyres_vertical(f: &mut Frame, area: Rect, app: &AppState) {
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
    
    render_tyre_widget(f, left_layout[0], 0, app, "FL");
    render_tyre_widget(f, left_layout[1], 2, app, "RL");
    render_tyre_widget(f, right_layout[0], 1, app, "FR");
    render_tyre_widget(f, right_layout[1], 3, app, "RR");
}

fn render_quick_info_vertical(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let info = vec![
        Line::from(vec![
            Span::styled(format!("{}: ", tr("lbl_fuel", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
            Span::styled(
                format!("{:.1}L", 
                    app.physics_mem.as_ref().map_or(0.0, |p| p.get().fuel)),
                Style::default().fg(get_fuel_color(app.engineer.stats.fuel_laps_remaining)),
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
            Span::styled(format!("{}: ", tr("lbl_tc", lang)), Style::default().fg(app.ui_state.get_color(&theme.text))),
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
            .title(tr("dash_quick", lang))
            .borders(Borders::ALL))
        .alignment(Alignment::Left);
    
    f.render_widget(info_widget, area);
}