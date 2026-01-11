use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap: &crate::analyzer::LapData,
    best_lap: Option<&crate::analyzer::LapData>,
) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == crate::config::Language::Russian;

    let lap_time_s = lap.lap_time_ms as f64 / 1000.0;
    let max_time_s = if lap_time_s < 1.0 { 60.0 } else { lap_time_s };
    let step = if max_time_s > 120.0 { 20.0 } else { 10.0 };

    let x_labels: Vec<Span<'_>> = (0..=(max_time_s / step).ceil() as i32)
        .map(|i| Span::raw(format!("{:.0}s", i as f64 * step)))
        .collect();

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let mut delta_data = vec![];
    let mut has_delta = false;

    if let Some(bl) = best_lap {
        if bl.lap_number != lap.lap_number
            && !lap.telemetry_trace.is_empty()
            && !bl.telemetry_trace.is_empty()
        {
            let min_len = lap.telemetry_trace.len().min(bl.telemetry_trace.len());
            for i in 0..min_len {
                let p_curr = &lap.telemetry_trace[i];
                let p_best = &bl.telemetry_trace[i];
                let dt = (p_curr.time_ms as f64 - p_best.time_ms as f64) / 1000.0;
                delta_data.push((p_curr.time_ms as f64 / 1000.0, dt));
            }
            has_delta = true;
        }
    }

    if !has_delta {
        delta_data = lap
            .telemetry_trace
            .iter()
            .map(|p| (p.time_ms as f64 / 1000.0, 0.0))
            .collect();
    }

    let delta_chart = Chart::new(vec![Dataset::default()
        .name(if is_ru {
            "Дельта (сек)"
        } else {
            "Time Delta (s)"
        })
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(if has_delta {
            Color::Yellow
        } else {
            Color::DarkGray
        }))
        .graph_type(GraphType::Line)
        .data(&delta_data)])
    .block(
        Block::default()
            .title(if is_ru {
                "Отставание от Лучшего (Время)"
            } else {
                "Time Delta vs Best"
            })
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
    )
    .x_axis(Axis::default().bounds([0.0, max_time_s]).labels(vec![]))
    .y_axis(Axis::default().bounds([-2.0, 2.0]).labels(vec![
        "-2.0".into(),
        "0.0".into(),
        "+2.0".into(),
    ]));
    f.render_widget(delta_chart, layout[0]);

    let speed_data: Vec<(f64, f64)> = lap
        .telemetry_trace
        .iter()
        .map(|p| (p.time_ms as f64 / 1000.0, p.speed as f64))
        .collect();

    let mut speed_datasets = vec![Dataset::default()
        .name(if is_ru {
            "Тек. Скор"
        } else {
            "Cur Speed"
        })
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .graph_type(GraphType::Line)
        .data(&speed_data)];

    let best_speed_data: Vec<(f64, f64)>;
    if let Some(bl) = best_lap {
        if bl.lap_number != lap.lap_number {
            best_speed_data = bl
                .telemetry_trace
                .iter()
                .map(|p| (p.time_ms as f64 / 1000.0, p.speed as f64))
                .collect();
            speed_datasets.push(
                Dataset::default()
                    .name(if is_ru { "Лучшая" } else { "Best" })
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(Color::Gray))
                    .graph_type(GraphType::Line)
                    .data(&best_speed_data),
            );
        }
    }

    let speed_chart = Chart::new(speed_datasets)
        .block(
            Block::default()
                .title(if is_ru {
                    "Скорость (км/ч)"
                } else {
                    "Speed (km/h)"
                })
                .borders(Borders::ALL),
        )
        .x_axis(Axis::default().bounds([0.0, max_time_s]).labels(vec![]))
        .y_axis(Axis::default().bounds([0.0, 350.0]).labels(vec![
            "0".into(),
            "150".into(),
            "300".into(),
        ]));
    f.render_widget(speed_chart, layout[1]);

    let gas_data: Vec<(f64, f64)> = lap
        .telemetry_trace
        .iter()
        .map(|p| (p.time_ms as f64 / 1000.0, p.gas as f64 * 100.0))
        .collect();
    let brake_data: Vec<(f64, f64)> = lap
        .telemetry_trace
        .iter()
        .map(|p| (p.time_ms as f64 / 1000.0, p.brake as f64 * 100.0))
        .collect();

    let inputs_chart = Chart::new(vec![
        Dataset::default()
            .name("Gas")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Green))
            .data(&gas_data),
        Dataset::default()
            .name("Brake")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Red))
            .data(&brake_data),
    ])
    .block(
        Block::default()
            .title(if is_ru {
                "Педали (%)"
            } else {
                "Pedals (%)"
            })
            .borders(Borders::ALL),
    )
    .x_axis(Axis::default().bounds([0.0, max_time_s]).labels(vec![]))
    .y_axis(
        Axis::default()
            .bounds([0.0, 100.0])
            .labels(vec!["0".into(), "100".into()]),
    );
    f.render_widget(inputs_chart, layout[2]);

    let steer_data: Vec<(f64, f64)> = lap
        .telemetry_trace
        .iter()
        .map(|p| (p.time_ms as f64 / 1000.0, p.steer as f64 * 360.0))
        .collect();
    let steer_chart = Chart::new(vec![Dataset::default()
        .name("Steer")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Yellow))
        .data(&steer_data)])
    .block(
        Block::default()
            .title(if is_ru {
                "Руль (град)"
            } else {
                "Steering (deg)"
            })
            .borders(Borders::ALL),
    )
    .x_axis(Axis::default().bounds([0.0, max_time_s]).labels(x_labels))
    .y_axis(Axis::default().bounds([-400.0, 400.0]).labels(vec![
        "-360".into(),
        "0".into(),
        "360".into(),
    ]));
    f.render_widget(steer_chart, layout[3]);
}
