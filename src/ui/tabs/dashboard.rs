use crate::ui::localization::tr;
use crate::ui::widgets::*;
use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render_horizontal(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    render_tyre_panel(f, layout[0], app);
    render_central_panel(f, layout[1], app);
    render_info_panel(f, layout[2], app);
}

pub fn render_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(area);

    render_tyres_vertical(f, layout[0], app);
    render_quick_info_vertical(f, layout[1], app);
}

fn render_tyre_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("dash_tyre_status", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(inner);

    let front = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(layout[0]);
    render_tyre_widget(f, front[0], 0, app, "FL");
    render_tyre_widget(f, front[1], 1, app, "FR");

    let rear = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(layout[1]);
    render_tyre_widget(f, rear[0], 2, app, "RL");
    render_tyre_widget(f, rear[1], 3, app, "RR");

    if let Some(phys) = &app.physics_mem {
        let data = phys.get();
        let avg_pressure: f32 = data.wheels_pressure.iter().sum::<f32>() / 4.0;
        let avg_temp: f32 = (0..4).map(|i| data.get_avg_tyre_temp(i)).sum::<f32>() / 4.0;

        let summary_text = vec![
            Line::from(vec![
                Span::styled("Avg Press: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.1} psi", avg_pressure),
                    Style::default().fg(get_pressure_color(avg_pressure)),
                ),
            ]),
            Line::from(vec![
                Span::styled("Avg Temp:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.0} °C", avg_temp),
                    Style::default().fg(get_tyre_color(avg_temp)),
                ),
            ]),
        ];

        let summary_block = Block::default()
            .borders(Borders::TOP)
            .padding(Padding::new(1, 0, 0, 0));
        f.render_widget(Paragraph::new(summary_text).block(summary_block), layout[2]);
    }
}

fn render_central_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;

    let phys_opt = app.physics_mem.as_ref().map(|p| p.get());
    let gfx_opt = app.graphics_mem.as_ref().map(|g| g.get());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" COCKPIT ")
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(phys) = phys_opt {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(4),
                Constraint::Length(2),
                Constraint::Min(0),
            ])
            .split(inner);

        let max_rpm = if app.session_info.max_rpm > 0 {
            app.session_info.max_rpm as f32
        } else {
            8000.0
        };
        let rpm_ratio = (phys.rpms as f32 / max_rpm).clamp(0.0, 1.0);

        let (rpm_color, label_text) = if rpm_ratio > 0.96 {
            (Color::Blue, "SHIFT NOW!".to_string())
        } else if rpm_ratio > 0.90 {
            (Color::Red, format!("{} RPM", phys.rpms))
        } else if rpm_ratio > 0.75 {
            (Color::Yellow, format!("{} RPM", phys.rpms))
        } else {
            (Color::Green, format!("{} RPM", phys.rpms))
        };

        let gauge_style = if rpm_ratio > 0.96 {
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(rpm_color).bg(Color::DarkGray)
        };

        f.render_widget(
            LineGauge::default()
                .ratio(rpm_ratio as f64)
                .label(label_text)
                .gauge_style(gauge_style),
            layout[0],
        );

        let main_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(layout[1]);

        let gear_char = match phys.gear {
            0 => "R".to_string(),
            1 => "N".to_string(),
            n => format!("{}", n - 1),
        };

        let speed_block = Block::default().borders(Borders::RIGHT);
        let speed_p = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{:.0}", phys.speed_kmh),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::ITALIC),
            )),
            Line::from(Span::styled("km/h", Style::default().fg(Color::DarkGray))),
        ])
        .alignment(Alignment::Center)
        .block(speed_block);

        let gear_p = Paragraph::new(gear_char)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(Block::default().padding(Padding::new(0, 0, 1, 0)));

        f.render_widget(speed_p, main_row[0]);
        f.render_widget(gear_p, main_row[1]);

        let pedals_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(layout[2]);

        render_mini_bar(f, pedals_layout[0], "C", phys.clutch, Color::Blue);
        render_mini_bar(f, pedals_layout[1], "B", phys.brake, Color::Red);
        render_mini_bar(f, pedals_layout[2], "T", phys.gas, Color::Green);

        let elec_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(layout[3]);

        let row1 = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(elec_layout[0]);
        let row2 = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(elec_layout[1]);

        let tc_level = phys.tc_level;
        let abs_level = phys.abs_level;
        let tc_cut = gfx_opt.map(|g| g.tccut).unwrap_or(0);
        let map_level = gfx_opt.map(|g| g.engine_map).unwrap_or(0);
        let bias = phys.brake_bias * 100.0;

        let tc_active = phys.tc_in_action > 0.0;
        let abs_active = phys.abs_in_action > 0.0;

        let tc_enabled_phys = phys.tc > 0.0;
        let abs_enabled_phys = phys.abs > 0.0;

        render_status_tile(f, row1[0], "TC", tc_level, tc_enabled_phys, tc_active);
        render_status_tile(f, row1[1], "ABS", abs_level, abs_enabled_phys, abs_active);

        if tc_cut > 0 {
            render_simple_tile(f, row2[0], "TC CUT", format!("{}", tc_cut), Color::Cyan);
        } else {
            render_simple_tile(f, row2[0], "MAP", format!("{}", map_level), Color::Magenta);
        }

        render_simple_tile(f, row2[1], "BIAS", format!("{:.1}%", bias), Color::Cyan);
    }
}

fn render_status_tile(
    f: &mut Frame<'_>,
    area: Rect,
    label: &str,
    level: i32,
    enabled_phys: bool,
    active: bool,
) {
    let (text, fg, bg) = if active {
        (
            if level > 0 {
                format!("{}", level)
            } else {
                "ACT".to_string()
            },
            Color::Black,
            Color::Yellow,
        )
    } else if level > 0 {
        (format!("{}", level), Color::Green, Color::Reset)
    } else if enabled_phys {
        ("ON".to_string(), Color::Green, Color::Reset)
    } else {
        ("OFF".to_string(), Color::DarkGray, Color::Reset)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(bg));

    let p = Paragraph::new(vec![
        Line::from(Span::styled(
            label,
            Style::default()
                .fg(if active { Color::Black } else { Color::Gray })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            text,
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(Alignment::Center)
    .block(block);

    f.render_widget(p, area);
}

fn render_simple_tile(f: &mut Frame<'_>, area: Rect, label: &str, value: String, color: Color) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let p = Paragraph::new(vec![
        Line::from(Span::styled(label, Style::default().fg(Color::Gray))),
        Line::from(Span::styled(
            value,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(Alignment::Center)
    .block(block);

    f.render_widget(p, area);
}

fn render_mini_bar(f: &mut Frame<'_>, area: Rect, label: &str, val: f32, color: Color) {
    let gauge = LineGauge::default()
        .block(Block::default().padding(Padding::new(1, 1, 0, 0)))
        .gauge_style(Style::default().fg(color))
        .ratio(val.clamp(0.0, 1.0) as f64)
        .label(label);
    f.render_widget(gauge, area);
}

fn render_info_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("dash_session", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let list = vec![
        Line::from(vec![
            Span::styled(
                format!("{}: ", tr("lbl_car", lang)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                &app.session_info.car_name,
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", tr("lbl_track", lang)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                &app.session_info.track_name,
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Time Left: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!(
                    "{:.1} min",
                    app.graphics_mem
                        .as_ref()
                        .map_or(0.0, |g| g.get().session_time_left / 60000.0)
                ),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{}: ", tr("lbl_fuel", lang)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!(
                    "{:.1} L",
                    app.physics_mem.as_ref().map_or(0.0, |p| p.get().fuel)
                ),
                Style::default().fg(get_fuel_color(app.engineer.stats.fuel_laps_remaining)),
            ),
        ]),
    ];

    f.render_widget(
        Paragraph::new(list).block(Block::default().padding(Padding::new(1, 1, 1, 1))),
        inner,
    );
}

fn render_tyres_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    render_tyre_panel(f, area, app);
}

fn render_quick_info_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    render_central_panel(f, area, app);
}

fn render_tyre_widget(f: &mut Frame<'_>, area: Rect, idx: usize, app: &AppState, label: &str) {
    if let Some(phys) = &app.physics_mem {
        let data = phys.get();
        let temp = data.get_avg_tyre_temp(idx);
        let press = data.wheels_pressure[idx];
        let wear = data.tyre_wear[idx];

        let block = Block::default().borders(Borders::ALL).title(label);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let text = vec![
            Line::from(Span::styled(
                format!("{:.0} C", temp),
                Style::default().fg(get_tyre_color(temp)),
            )),
            Line::from(Span::styled(
                format!("{:.1} psi", press),
                Style::default().fg(get_pressure_color(press)),
            )),
            Line::from(Span::styled(
                format!("{:.0}%", wear),
                Style::default().fg(get_wear_color(wear)),
            )),
        ];
        f.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner);
    }
}

fn get_temp_color(temp: f32) -> Color {
    if temp < 70.0 {
        Color::Blue
    } else if temp > 100.0 {
        Color::Red
    } else {
        Color::Green
    }
}
fn get_tyre_color(temp: f32) -> Color {
    get_temp_color(temp)
}
fn get_pressure_color(press: f32) -> Color {
    if (press - 27.5).abs() < 1.5 {
        Color::Green
    } else {
        Color::Yellow
    }
}
fn get_wear_color(wear: f32) -> Color {
    if wear > 96.0 {
        Color::Green
    } else if wear > 94.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}
fn get_fuel_color(laps: f32) -> Color {
    if laps < 2.0 {
        Color::Red
    } else if laps < 5.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}
