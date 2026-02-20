use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState, lap: &ac_core::analyzer::LapData) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let max_time_s = (lap.lap_time_ms as f64 / 1000.0).max(1.0);
    let x_labels: Vec<Span<'_>> = (0..=(max_time_s / 15.0) as i32)
        .map(|i| Span::raw(format!("{:.0}s", i as f64 * 15.0)))
        .collect();

    let slip_data: Vec<(f64, f64)> = lap
        .telemetry_trace
        .iter()
        .map(|p| (p.time_ms as f64 / 1000.0, p.slip_avg as f64))
        .collect();

    let tc_data: Vec<(f64, f64)> = lap
        .telemetry_trace
        .iter()
        .map(|p| (p.time_ms as f64 / 1000.0, p.gas as f64 * 5.0))
        .collect();

    let chart = Chart::new(vec![
        Dataset::default()
            .name(if is_ru {
                "Проскальзывание"
            } else {
                "Slip Ratio"
            })
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Magenta))
            .graph_type(GraphType::Line)
            .data(&slip_data),
        Dataset::default()
            .name(if is_ru { "Газ (Ref)" } else { "Gas (Ref)" })
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::DarkGray))
            .graph_type(GraphType::Line)
            .data(&tc_data),
    ])
    .block(
        Block::default()
            .title(if is_ru {
                "Потеря Сцепления (Slip vs Time)"
            } else {
                "Traction Loss"
            })
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
    )
    .x_axis(Axis::default().bounds([0.0, max_time_s]).labels(x_labels))
    .y_axis(
        Axis::default()
            .bounds([0.0, 10.0])
            .labels(vec!["0".into(), "5".into(), "10".into()]),
    );

    f.render_widget(chart, layout[0]);

    let bottom_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(layout[1]);

    let block_stats = Block::default()
        .title(if is_ru {
            "Анализ Трекшена"
        } else {
            "Traction Stats"
        })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let avg_slip: f64 = if !lap.telemetry_trace.is_empty() {
        lap.telemetry_trace
            .iter()
            .map(|p| p.slip_avg as f64)
            .sum::<f64>()
            / lap.telemetry_trace.len() as f64
    } else {
        0.0
    };

    let stats = vec![
        Row::new(vec![
            Cell::from(if is_ru {
                "Использ. Сцепления"
            } else {
                "Grip Usage"
            }),
            Cell::from(format!("{:.1}%", lap.grip_usage_percent))
                .style(Style::default().fg(Color::Green)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Агрессия на выходе"
            } else {
                "Exit Aggression"
            }),
            Cell::from(format!("{:.1}%", lap.radar_stats.aggression * 100.0))
                .style(Style::default().fg(Color::Red)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Плавность Газа"
            } else {
                "Throttle Smooth"
            }),
            Cell::from(format!("{:.1}%", lap.throttle_smoothness))
                .style(Style::default().fg(Color::Cyan)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Стабильность (TC)"
            } else {
                "Stability (TC)"
            }),
            Cell::from(format!("{:.1}%", (1.0 - avg_slip / 10.0) * 100.0))
                .style(Style::default().fg(Color::Yellow)),
        ]),
    ];
    f.render_widget(
        Table::new(
            stats,
            [Constraint::Percentage(70), Constraint::Percentage(30)],
        )
        .block(block_stats),
        bottom_layout[0],
    );

    let throttle_g_data: Vec<(f64, f64)> = lap
        .telemetry_trace
        .iter()
        .filter(|p| p.gas > 0.05)
        .step_by(5)
        .map(|p| (p.lat_g.abs() as f64, p.gas as f64 * 100.0))
        .collect();

    let scatter = Chart::new(vec![Dataset::default()
        .name("Gas vs Lat G")
        .marker(symbols::Marker::Dot)
        .style(Style::default().fg(Color::Yellow))
        .graph_type(GraphType::Scatter)
        .data(&throttle_g_data)])
    .block(
        Block::default()
            .title(if is_ru {
                "Газ в Повороте"
            } else {
                "Throttle in Corner"
            })
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
    )
    .x_axis(Axis::default().bounds([0.0, 3.0]).labels(vec![
        "0G".into(),
        "1.5G".into(),
        "3G".into(),
    ]))
    .y_axis(
        Axis::default()
            .bounds([0.0, 100.0])
            .labels(vec!["0%".into(), "100%".into()]),
    );

    f.render_widget(scatter, bottom_layout[1]);
}
