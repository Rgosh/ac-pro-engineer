use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState, lap: &ac_core::analyzer::LapData) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(area);

    let max_time_s = (lap.lap_time_ms as f64 / 1000.0).max(1.0);
    let x_labels: Vec<Span<'_>> = (0..=(max_time_s / 15.0) as i32)
        .map(|i| Span::raw(format!("{:.0}s", i as f64 * 15.0)))
        .collect();

    let rpm_data: Vec<(f64, f64)> = lap
        .telemetry_trace
        .iter()
        .map(|p| {
            let rpm_val = p.speed as f64 * 25.0 + 1000.0;
            (p.time_ms as f64 / 1000.0, rpm_val)
        })
        .collect();

    let chart_rpm = Chart::new(vec![Dataset::default()
        .name("RPM")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Red))
        .graph_type(GraphType::Line)
        .data(&rpm_data)])
    .block(
        Block::default()
            .title(if is_ru {
                "Обороты Двигателя (RPM)"
            } else {
                "Engine RPM"
            })
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
    )
    .x_axis(Axis::default().bounds([0.0, max_time_s]).labels(x_labels))
    .y_axis(Axis::default().bounds([0.0, 9000.0]).labels(vec![
        "0".into(),
        "4k".into(),
        "8k".into(),
    ]));

    f.render_widget(chart_rpm, layout[0]);

    let mut gear_counts = vec![0u64; 9];
    let mut total_points = 0;

    for p in &lap.telemetry_trace {
        let g = if p.gear < 0 { 0 } else { p.gear as usize };
        if g < gear_counts.len() {
            gear_counts[g] += 1;
            total_points += 1;
        }
    }

    let gear_labels = ["N", "1", "2", "3", "4", "5", "6", "7", "8"];
    let mut bar_data = Vec::new();
    for i in 0..9 {
        if gear_counts[i] > 0 {
            let pct = (gear_counts[i] as f64 / total_points as f64 * 100.0) as u64;
            bar_data.push((gear_labels[i], pct));
        }
    }

    let barchart = BarChart::default()
        .block(
            Block::default()
                .title(if is_ru {
                    "Распределение Передач (%)"
                } else {
                    "Gear Distribution (%)"
                })
                .borders(Borders::ALL),
        )
        .data(&bar_data.iter().map(|(s, v)| (*s, *v)).collect::<Vec<_>>())
        .bar_width(5)
        .bar_gap(2)
        .value_style(Style::default().fg(Color::Black).bg(Color::Yellow))
        .bar_style(Style::default().fg(Color::Yellow));

    f.render_widget(barchart, layout[1]);

    let bottom_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[2]);

    let stats_block = Block::default()
        .title(if is_ru {
            "Эффективность"
        } else {
            "Efficiency"
        })
        .borders(Borders::ALL);

    let stats_text = vec![
        Row::new(vec![
            Cell::from(if is_ru {
                "Всего переключений"
            } else {
                "Total Shifts"
            }),
            Cell::from(lap.gear_shifts.to_string()).style(Style::default().fg(Color::White)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Ср. Расход (л/круг)"
            } else {
                "Fuel/Lap (Est)"
            }),
            Cell::from(format!("{:.2}", lap.fuel_used)).style(Style::default().fg(Color::Red)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Время в пол (WOT)"
            } else {
                "Time @ WOT"
            }),
            Cell::from(format!("{:.1}%", lap.full_throttle_percent))
                .style(Style::default().fg(Color::Green)),
        ]),
    ];

    f.render_widget(
        Table::new(
            stats_text,
            [Constraint::Percentage(70), Constraint::Percentage(30)],
        )
        .block(stats_block),
        bottom_split[0],
    );

    let fuel_data = vec![(0.0, 10.0), (max_time_s, (10.0 - lap.fuel_used) as f64)];

    let fuel_chart = Chart::new(vec![Dataset::default()
        .name("Fuel")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Magenta))
        .data(&fuel_data)])
    .block(
        Block::default()
            .title(if is_ru {
                "Топливо (кг)"
            } else {
                "Fuel Level"
            })
            .borders(Borders::ALL),
    )
    .x_axis(Axis::default().bounds([0.0, max_time_s]))
    .y_axis(
        Axis::default()
            .bounds([0.0, 15.0])
            .labels(vec!["0".into(), "15".into()]),
    );

    f.render_widget(fuel_chart, bottom_split[1]);
}
