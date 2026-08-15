use crate::AppState;
use ac_core::games::reading::{COORD_X, COORD_Z};
use ac_core::i18n::Translate;
use ratatui::widgets::canvas::{Canvas, Circle, Line as CanvasLine, Points};
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;

    if app.car_history.is_empty() {
        let block = Block::default()
            .title("TELEMETRY".tr_lang(lang).to_string())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        let text = Paragraph::new("Waiting for data...".tr_lang(lang).to_string())
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
        .car_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.speed_kmh as f64))
        .collect();
    let rpm_data: Vec<(f64, f64)> = app
        .car_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.rpm as f64 / 25.0))
        .collect();

    let chart = Chart::new(vec![
        Dataset::default()
            .name("SPD".tr_lang(lang).to_string())
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
            .title("Speed & RPM".tr_lang(lang).to_string())
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
        .car_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.throttle as f64 * 100.0))
        .collect();
    let brake: Vec<(f64, f64)> = app
        .car_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.brake as f64 * 100.0))
        .collect();

    let chart = Chart::new(vec![
        Dataset::default()
            .name("THR".tr_lang(lang).to_string())
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Green))
            .data(&gas),
        Dataset::default()
            .name("BRK".tr_lang(lang).to_string())
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Red))
            .data(&brake),
    ])
    .block(
        Block::default()
            .title("Pedal Inputs".tr_lang(lang).to_string())
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
        .car_history
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.steer_angle as f64 * 360.0))
        .collect();

    let chart = Chart::new(vec![
        Dataset::default()
            .name("Angle".tr_lang(lang).to_string())
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::White))
            .data(&steer),
    ])
    .block(
        Block::default()
            .title("Steering Angle".tr_lang(lang).to_string())
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
        .title("Track Map".tr_lang(lang).to_string())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let (trace_points, min_x, max_x, min_y, max_y) = if let Some(best_idx) =
        app.analyzer.best_lap_index
        && let Some(lap) = app.analyzer.laps.get(best_idx)
        && !lap.telemetry_trace.is_empty()
    {
        let min_x = if lap.bounds_min_x.is_finite() && lap.bounds_min_x.abs() < 1e6 {
            lap.bounds_min_x as f64
        } else {
            -500.0
        };
        let max_x = if lap.bounds_max_x.is_finite() && lap.bounds_max_x.abs() < 1e6 {
            lap.bounds_max_x as f64
        } else {
            500.0
        };
        let min_y = if lap.bounds_min_y.is_finite() && lap.bounds_min_y.abs() < 1e6 {
            lap.bounds_min_y as f64
        } else {
            -500.0
        };
        let max_y = if lap.bounds_max_y.is_finite() && lap.bounds_max_y.abs() < 1e6 {
            lap.bounds_max_y as f64
        } else {
            500.0
        };
        let points: Vec<(f64, f64)> = lap
            .telemetry_trace
            .iter()
            .map(|p| (p.x as f64, p.y as f64))
            .collect();
        (points, min_x, max_x, min_y, max_y)
    } else if app.is_demo_mode || !app.car_history.is_empty() {
        let mut points = Vec::with_capacity(100);
        for i in 0..100 {
            let angle = (i as f64 / 100.0) * std::f64::consts::TAU;
            let rx = 400.0 * angle.cos() + 50.0 * (2.0 * angle).cos();
            let ry = 250.0 * angle.sin() + 30.0 * (3.0 * angle).sin();
            points.push((rx, ry));
        }
        (points, -500.0, 500.0, -350.0, 350.0)
    } else {
        let p = Paragraph::new("Drive a lap to generate map...".tr_lang(lang).to_string())
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    };

    let diff_x = (max_x - min_x).max(10.0);
    let diff_y = (max_y - min_y).max(10.0);
    let margin_x = diff_x * 0.1;
    let margin_y = diff_y * 0.1;

    let x_bounds = [min_x - margin_x, max_x + margin_x];
    let y_bounds = [min_y - margin_y, max_y + margin_y];

    let car_pos = if let Some(gfx) = app.session() {
        let cx = gfx.car_position_m[COORD_X] as f64;
        let cy = gfx.car_position_m[COORD_Z] as f64;
        if cx.is_finite() && cy.is_finite() && (cx != 0.0 || cy != 0.0) {
            Some((cx, cy))
        } else if app.is_demo_mode {
            let progress = gfx.track_position.clamp(0.0, 1.0) as f64;
            let angle = progress * std::f64::consts::TAU;
            let cx = 400.0 * angle.cos() + 50.0 * (2.0 * angle).cos();
            let cy = 250.0 * angle.sin() + 30.0 * (3.0 * angle).sin();
            Some((cx, cy))
        } else {
            None
        }
    } else {
        None
    };

    let canvas = Canvas::default()
        .block(block)
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .paint(move |ctx| {
            for &(px, py) in &trace_points {
                ctx.draw(&Points {
                    coords: &[(px, py)],
                    color: Color::DarkGray,
                });
            }

            if let Some((car_x, car_y)) = car_pos {
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
}

fn render_friction_circle(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;

    if let Some(data) = app.car() {
        let lat = data.acc_g[0] as f64;
        let lon = data.acc_g[2] as f64;

        let canvas = Canvas::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Friction Circle (G-G)".tr_lang(lang).to_string())
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

                let history_len = app.car_history.len();
                let trail_count = 30;
                if history_len > trail_count {
                    for i in 0..trail_count {
                        let p = &app.car_history[history_len - 1 - i];

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
        .title("Live Telemetry".tr_lang(lang).to_string())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(phys) = app.car() {
        let rows = vec![
            Row::new(vec![
                Cell::from("Speed").style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:.0} km/h", phys.speed_kmh)).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Row::new(vec![
                Cell::from("Gear").style(Style::default().fg(Color::Gray)),
                Cell::from(crate::ui::widgets::gear_label(phys.gear))
                    .style(Style::default().fg(Color::Yellow)),
            ]),
            Row::new(vec![
                Cell::from("Lat G").style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:.2}", phys.acc_g[0]))
                    .style(Style::default().fg(Color::White)),
            ]),
            Row::new(vec![
                Cell::from("Lon G").style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:.2}", phys.acc_g[2]))
                    .style(Style::default().fg(Color::White)),
            ]),
            Row::new(vec![
                Cell::from("Steer").style(Style::default().fg(Color::Gray)),
                Cell::from(format!("{:.0}°", phys.steer_angle * 360.0))
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
