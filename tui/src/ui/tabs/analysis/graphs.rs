use crate::AppState;
use ac_core::i18n::Translate;
use ratatui::{prelude::*, widgets::*};

pub fn render(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap: &ac_core::analyzer::LapData,
    best_lap: Option<&ac_core::analyzer::LapData>,
) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

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

    // Cached on the pair of lap numbers. Both laps are finished, so the series
    // cannot change between frames — and computing it resamples two traces of
    // up to 7200 points, cloning and sorting each.
    let mut cache = app.ui_state.analysis.delta_cache.borrow_mut();
    let mut delta_data: Vec<(f64, f64)> = Vec::new();
    let mut has_delta = false;

    if let Some(bl) = best_lap
        && bl.lap_number != lap.lap_number
        && !lap.telemetry_trace.is_empty()
        && !bl.telemetry_trace.is_empty()
    {
        let series = cache.get_or_compute(lap.lap_number, bl.lap_number, || {
            ac_core::analyzer::LapComparison::delta_by_distance(
                &lap.telemetry_trace,
                &bl.telemetry_trace,
                0.002,
            )
        });
        has_delta = !series.is_empty();
        if has_delta {
            delta_data = series.to_vec();
        }
    }

    if !has_delta {
        delta_data = lap
            .telemetry_trace
            .iter()
            .map(|p| (p.time_ms as f64 / 1000.0, 0.0))
            .collect();
    }
    drop(cache);

    let delta_chart = Chart::new(vec![
        Dataset::default()
            .name("Time Delta (s)".tr(is_ru))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(if has_delta {
                Color::Yellow
            } else {
                Color::DarkGray
            }))
            .graph_type(GraphType::Line)
            .data(&delta_data),
    ])
    .block(
        Block::default()
            .title("Time Delta vs Best".tr(is_ru))
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

    let mut speed_datasets = vec![
        Dataset::default()
            .name("Cur Speed".tr(is_ru))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .graph_type(GraphType::Line)
            .data(&speed_data),
    ];

    let best_speed_data: Vec<(f64, f64)>;
    if let Some(bl) = best_lap
        && bl.lap_number != lap.lap_number
    {
        best_speed_data = bl
            .telemetry_trace
            .iter()
            .map(|p| (p.time_ms as f64 / 1000.0, p.speed as f64))
            .collect();
        speed_datasets.push(
            Dataset::default()
                .name("Best".tr(is_ru))
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(Color::Gray))
                .graph_type(GraphType::Line)
                .data(&best_speed_data),
        );
    }

    let speed_chart = Chart::new(speed_datasets)
        .block(
            Block::default()
                .title("Speed (km/h)".tr(is_ru))
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
            .title("Pedals (%)".tr(is_ru))
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
    let steer_chart = Chart::new(vec![
        Dataset::default()
            .name("Steer")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Yellow))
            .data(&steer_data),
    ])
    .block(
        Block::default()
            .title("Steering (deg)".tr(is_ru))
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
