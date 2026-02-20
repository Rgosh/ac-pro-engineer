use crate::ui::localization::tr;
use crate::AppState;
use ratatui::widgets::canvas::{Canvas, Circle, Line as CanvasLine, Points};
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;

    if app.physics_history.is_empty() {
        let block = Block::default()
            .title(tr("tab_tele", lang))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        let text = Paragraph::new(tr("no_data", lang))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(text, area);
        return;
    }

    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let graphs_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(main_layout[0]);

    render_speed_rpm_graph(f, graphs_layout[0], app);
    render_inputs_graph(f, graphs_layout[1], app);
    render_steering_graph(f, graphs_layout[2], app);

    let visual_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(main_layout[1]);

    render_track_map(f, visual_layout[0], app);
    render_friction_circle(f, visual_layout[1], app);
    render_live_stats(f, visual_layout[2], app);
}

fn render_speed_rpm_graph(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    let min_x = 0.0;
    let max_x = app.config.history_size as f64;

    let speed_data: Vec<(f64, f64)> = app
        .physics_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.speed_kmh as f64))
        .collect();
    let rpm_data: Vec<(f64, f64)> = app
        .physics_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.rpms as f64 / 25.0))
        .collect();

    let chart = Chart::new(vec![
        Dataset::default()
            .name(tr("lbl_speed", lang))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&speed_data),
        Dataset::default()
            .name("RPM/25")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::DarkGray))
            .data(&rpm_data),
    ])
    .block(
        Block::default()
            .title(tr("graph_speed_rpm", lang))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
    )
    .x_axis(Axis::default().bounds([min_x, max_x]))
    .y_axis(
        Axis::default()
            .bounds([0.0, 320.0])
            .labels(vec![Span::raw("0"), Span::raw("320")]),
    );

    f.render_widget(chart, area);
}

fn render_inputs_graph(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    let min_x = 0.0;
    let max_x = app.config.history_size as f64;

    let gas: Vec<(f64, f64)> = app
        .physics_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.gas as f64 * 100.0))
        .collect();
    let brake: Vec<(f64, f64)> = app
        .physics_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.brake as f64 * 100.0))
        .collect();

    let chart = Chart::new(vec![
        Dataset::default()
            .name(tr("lbl_throttle", lang))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Green))
            .data(&gas),
        Dataset::default()
            .name(tr("lbl_brake", lang))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Red))
            .data(&brake),
    ])
    .block(
        Block::default()
            .title(tr("graph_inputs", lang))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
    )
    .x_axis(Axis::default().bounds([min_x, max_x]))
    .y_axis(Axis::default().bounds([0.0, 100.0]));

    f.render_widget(chart, area);
}

fn render_steering_graph(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    let min_x = 0.0;
    let max_x = app.config.history_size as f64;

    let steer: Vec<(f64, f64)> = app
        .physics_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.steer_angle as f64 * 360.0))
        .collect();

    let chart = Chart::new(vec![Dataset::default()
        .name(tr("lbl_steer", lang))
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::White))
        .data(&steer)])
    .block(
        Block::default()
            .title(tr("graph_steering", lang))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
    )
    .x_axis(Axis::default().bounds([min_x, max_x]))
    .y_axis(Axis::default().bounds([-400.0, 400.0]));

    f.render_widget(chart, area);
}

fn render_track_map(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;

    let block = Block::default()
        .title(tr("tele_map", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    if let Some(best_idx) = app.analyzer.best_lap_index {
        let lap = &app.analyzer.laps[best_idx];

        let margin_x = (lap.bounds_max_x - lap.bounds_min_x) * 0.1;
        let margin_y = (lap.bounds_max_y - lap.bounds_min_y) * 0.1;

        let x_bounds = [
            (lap.bounds_min_x - margin_x) as f64,
            (lap.bounds_max_x + margin_x) as f64,
        ];

        let y_bounds = [
            (lap.bounds_min_y - margin_y) as f64,
            (lap.bounds_max_y + margin_y) as f64,
        ];

        let canvas = Canvas::default()
            .block(block)
            .x_bounds(x_bounds)
            .y_bounds(y_bounds)
            .paint(move |ctx| {
                for p in &lap.telemetry_trace {
                    ctx.draw(&Points {
                        coords: &[(p.x as f64, p.y as f64)],
                        color: Color::DarkGray,
                    });
                }

                if let Some(gfx) = &app.graphics_mem {
                    let g = gfx.get();

                    let car_x = g.car_coordinates[0][0] as f64;
                    let car_y = g.car_coordinates[0][2] as f64;

                    let scale = (x_bounds[1] - x_bounds[0]) / 50.0;

                    ctx.draw(&Circle {
                        x: car_x,
                        y: car_y,
                        radius: scale,
                        color: Color::Red,
                    });
                }
            });

        f.render_widget(canvas, area);
    } else {
        let p = Paragraph::new(tr("tele_map_waiting", lang))
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
    }
}

fn render_friction_circle(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;

    if let Some(phys) = &app.physics_mem {
        let data = phys.get();

        let lat = data.acc_g[0] as f64;
        let lon = data.acc_g[2] as f64;

        let canvas = Canvas::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(tr("tele_friction", lang))
                    .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
            )
            .x_bounds([-3.0, 3.0])
            .y_bounds([-3.0, 3.0])
            .paint(move |ctx| {
                ctx.draw(&Circle {
                    x: 0.0,
                    y: 0.0,
                    radius: 1.0,
                    color: Color::DarkGray,
                });
                ctx.draw(&Circle {
                    x: 0.0,
                    y: 0.0,
                    radius: 2.0,
                    color: Color::DarkGray,
                });

                ctx.draw(&CanvasLine {
                    x1: -3.0,
                    y1: 0.0,
                    x2: 3.0,
                    y2: 0.0,
                    color: Color::DarkGray,
                });
                ctx.draw(&CanvasLine {
                    x1: 0.0,
                    y1: -3.0,
                    x2: 0.0,
                    y2: 3.0,
                    color: Color::DarkGray,
                });

                let history_len = app.physics_history.len();
                let trail_count = 30;
                if history_len > trail_count {
                    for i in 0..trail_count {
                        let p = &app.physics_history[history_len - 1 - i];

                        ctx.draw(&Points {
                            coords: &[(p.acc_g[0] as f64, p.acc_g[2] as f64)],
                            color: if i < 5 { Color::Yellow } else { Color::Gray },
                        });
                    }
                }

                let g_sum = (lat * lat + lon * lon).sqrt();
                let color = if g_sum > 2.5 {
                    Color::Red
                } else if g_sum > 1.5 {
                    Color::LightRed
                } else {
                    Color::Yellow
                };
                ctx.draw(&Circle {
                    x: lat,
                    y: lon,
                    radius: 0.25,
                    color,
                });
            });

        f.render_widget(canvas, area);
    }
}

fn render_live_stats(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;

    let block = Block::default()
        .title(tr("tele_live", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(phys) = &app.physics_mem {
        let p = phys.get();
        let rows = vec![
            Row::new(vec![
                Cell::from("Speed").style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:.0} km/h", p.speed_kmh)).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Row::new(vec![
                Cell::from("Gear").style(Style::default().fg(Color::Gray)),
                Cell::from(
                    (if p.gear == 0 {
                        "R".into()
                    } else if p.gear == 1 {
                        "N".into()
                    } else {
                        (p.gear - 1).to_string()
                    })
                    .to_string(),
                )
                .style(Style::default().fg(Color::Yellow)),
            ]),
            Row::new(vec![
                Cell::from("Lat G").style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:.2}", p.acc_g[0])).style(Style::default().fg(Color::White)),
            ]),
            Row::new(vec![
                Cell::from("Lon G").style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:.2}", p.acc_g[2])).style(Style::default().fg(Color::White)),
            ]),
            Row::new(vec![
                Cell::from("Steer").style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:.0}°", p.steer_angle * 360.0))
                    .style(Style::default().fg(Color::White)),
            ]),
        ];

        let table = Table::new(
            rows,
            [Constraint::Percentage(40), Constraint::Percentage(60)],
        );
        f.render_widget(table, inner);
    }
}
