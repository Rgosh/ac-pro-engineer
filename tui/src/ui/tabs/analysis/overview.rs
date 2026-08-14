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
    // The ambient temperatures were rendered with a hardcoded "C" while the
    // user may have Fahrenheit selected.
    let fmt = app.config.formatter();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area);

    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)))
        .title("LAP OVERVIEW".tr(is_ru));

    let min = lap.lap_time_ms / 60000;
    let sec = (lap.lap_time_ms % 60000) / 1000;
    let ms = lap.lap_time_ms % 1000;
    let time_str = format!("{}:{:02}.{:03}", min, sec, ms);

    let diff_text = if let Some(best) = best_lap {
        let diff = lap.lap_time_ms - best.lap_time_ms;
        let sign = if diff > 0 { "+" } else { "-" };
        let abs_diff = diff.abs();
        let color = if diff > 0 { Color::Red } else { Color::Green };
        Span::styled(
            format!("Delta: {}{}.{:03}", sign, abs_diff / 1000, abs_diff % 1000),
            Style::default().fg(color),
        )
    } else {
        Span::raw("Session Best".tr(is_ru))
    };

    let valid_style = if lap.valid {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };
    let valid_text = if lap.valid { "VALID" } else { "INVALID" };

    let header_content = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(chunks[0].inner(&Margin {
            vertical: 1,
            horizontal: 1,
        }));

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(format!("Lap {} | ", lap.lap_number + 1)),
            Span::styled(valid_text, valid_style),
        ])),
        header_content[0],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            time_str,
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow)
                .bg(Color::Black),
        ))
        .alignment(Alignment::Center),
        header_content[1],
    );
    f.render_widget(
        Paragraph::new(diff_text).alignment(Alignment::Right),
        header_content[2],
    );
    f.render_widget(header_block, chunks[0]);

    let row1 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    // Taken from the analyzer rather than recomputed here. The local version
    // was a plain `.min()` over the raw sector values, which includes the
    // zeroes left by a lap whose split was never captured and by the unused
    // third slot of a two-sector track — so a single such lap pinned that
    // best sector to 0.000 and made the theoretical best a number no car
    // could set.
    let best = app.analyzer.best_sectors_ms();
    let theoretical_best = app.analyzer.theoretical_best_lap_ms();

    // A sector with no recorded best, and the lap-vs-best columns that depend
    // on one, render as a dash. Printing 0.000 there would read as a sector
    // time rather than as an absence.
    const NO_TIME: &str = "  ---";
    let secs = |ms: i32| format!("{:.3}", ms as f64 / 1000.0);

    let mut sec_rows: Vec<Row<'_>> = (0..3)
        .map(|i| {
            let driven = lap.sectors[i];
            let (best_cell, diff_cell) = match best[i] {
                Some(best_ms) => (
                    Cell::from(secs(best_ms)).style(Style::default().fg(Color::Cyan)),
                    Cell::from(secs(driven - best_ms)).style(Style::default().fg(
                        if driven <= best_ms {
                            Color::Green
                        } else {
                            Color::Red
                        },
                    )),
                ),
                None => (
                    Cell::from(NO_TIME).style(Style::default().fg(Color::DarkGray)),
                    Cell::from(NO_TIME).style(Style::default().fg(Color::DarkGray)),
                ),
            };

            Row::new(vec![
                Cell::from(format!("S{}", i + 1)),
                Cell::from(if driven > 0 {
                    secs(driven)
                } else {
                    NO_TIME.to_string()
                }),
                best_cell,
                diff_cell,
            ])
        })
        .collect();

    let (optimal_cell, optimal_diff) = match theoretical_best {
        Some(best_ms) => (
            Cell::from(secs(best_ms)).style(Style::default().fg(Color::Magenta)),
            Cell::from(secs(lap.lap_time_ms - best_ms)).style(Style::default().fg(Color::Yellow)),
        ),
        // Every sector needs a time before the sum means anything.
        None => (
            Cell::from(NO_TIME).style(Style::default().fg(Color::DarkGray)),
            Cell::from(NO_TIME).style(Style::default().fg(Color::DarkGray)),
        ),
    };

    sec_rows.push(Row::new(vec![
        Cell::from("Optimal".tr(is_ru)),
        Cell::from("----"),
        optimal_cell,
        optimal_diff,
    ]));

    let sec_table = Table::new(sec_rows, [Constraint::Ratio(1, 4); 4])
        .header(
            Row::new(vec!["Sec", "Time", "Best", "Diff"]).style(Style::default().fg(Color::Gray)),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Sector Analysis".tr(is_ru)),
        );
    f.render_widget(sec_table, row1[0]);

    let score_block = Block::default()
        .borders(Borders::ALL)
        .title("Driving Evaluation".tr(is_ru));
    let score_area = score_block.inner(row1[1]);
    f.render_widget(score_block, row1[1]);

    let score_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1); 4])
        .split(score_area);

    let stability_score =
        (100.0_f64 - (lap.oversteer_count as f64 * 10.0) - (lap.lockup_count as f64 * 10.0))
            .clamp(0.0, 100.0);
    let aggression_score = (lap.full_throttle_percent as f64).clamp(0.0, 100.0);
    let grip_score = (lap.grip_usage_percent as f64).clamp(0.0, 100.0);
    let overall_score =
        (stability_score * 0.4 + aggression_score * 0.3 + grip_score * 0.3).clamp(0.0, 100.0);

    let make_gauge = |label: &str, val: f64, color: Color| {
        Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(color))
            .ratio(crate::ui::widgets::safe_ratio(val / 100.0))
            .label(format!("{}: {:.0}/100", label, val))
    };

    f.render_widget(
        make_gauge("Overall Score".tr(is_ru), overall_score, Color::Magenta),
        score_layout[0],
    );
    f.render_widget(
        make_gauge("Stability".tr(is_ru), stability_score, Color::Green),
        score_layout[1],
    );
    f.render_widget(
        make_gauge("Aggression|bare".tr(is_ru), aggression_score, Color::Red),
        score_layout[2],
    );
    f.render_widget(
        make_gauge("Grip Usage|shorter".tr(is_ru), grip_score, Color::Cyan),
        score_layout[3],
    );

    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(chunks[2]);

    let car_schema_block = Block::default()
        .borders(Borders::ALL)
        .title("Car (T/B/W)".tr(is_ru));
    let car_area = car_schema_block.inner(row2[0]);
    f.render_widget(car_schema_block, row2[0]);

    let wheels_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(car_area);

    let front_wheels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(wheels_layout[0]);
    let rear_wheels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(wheels_layout[1]);

    let render_wheel = |f: &mut Frame<'_>, area: Rect, name: &str, idx: usize| {
        let temp = *lap.avg_tyre_temp.get(idx).unwrap_or(&0.0);
        let brake = *lap.max_brake_temp.get(idx).unwrap_or(&0.0);

        let color = if temp > 100.0 {
            Color::Red
        } else if temp < 70.0 {
            Color::Blue
        } else {
            Color::Green
        };

        let text = vec![
            Line::from(Span::styled(
                name,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled(format!("{:.0}C ", temp), Style::default().fg(color)),
                Span::styled(
                    format!("{:.0}C", brake),
                    Style::default().fg(if brake > 600.0 {
                        Color::Red
                    } else {
                        Color::Yellow
                    }),
                ),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        f.render_widget(
            Paragraph::new(text)
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
    };

    render_wheel(f, front_wheels[0], "FL", 0);
    render_wheel(f, front_wheels[1], "FR", 1);
    render_wheel(f, rear_wheels[0], "RL", 2);
    render_wheel(f, rear_wheels[1], "RR", 3);

    let ms_block = Block::default()
        .borders(Borders::ALL)
        .title("Micro-Sectors (Delta)".tr(is_ru));

    let mut ms_rows = vec![];

    if let Some(best) = best_lap {
        if !lap.telemetry_trace.is_empty() && !best.telemetry_trace.is_empty() {
            let num_sectors = 8;
            for i in 0..num_sectors {
                let start_dist = i as f32 / num_sectors as f32;
                let end_dist = (i + 1) as f32 / num_sectors as f32;

                let cur_start = lap
                    .telemetry_trace
                    .iter()
                    .find(|p| p.distance >= start_dist)
                    .map(|p| p.time_ms)
                    .unwrap_or(0);
                let cur_end = lap
                    .telemetry_trace
                    .iter()
                    .find(|p| p.distance >= end_dist)
                    .map(|p| p.time_ms)
                    .unwrap_or(lap.lap_time_ms);
                let cur_time = cur_end - cur_start;

                let best_start = best
                    .telemetry_trace
                    .iter()
                    .find(|p| p.distance >= start_dist)
                    .map(|p| p.time_ms)
                    .unwrap_or(0);
                let best_end = best
                    .telemetry_trace
                    .iter()
                    .find(|p| p.distance >= end_dist)
                    .map(|p| p.time_ms)
                    .unwrap_or(best.lap_time_ms);
                let best_time = best_end - best_start;

                let diff = (cur_time as f32 - best_time as f32) / 1000.0;

                let diff_str = if diff > 0.0 {
                    format!("+{:.3}s", diff)
                } else {
                    format!("{:.3}s", diff)
                };

                let color = if diff > 0.0 {
                    Color::Red
                } else if diff < 0.0 {
                    Color::Green
                } else {
                    Color::DarkGray
                };

                ms_rows.push(Row::new(vec![
                    Cell::from(format!("MS {}", i + 1)),
                    Cell::from(diff_str).style(Style::default().fg(color)),
                ]));
            }
        } else {
            ms_rows.push(Row::new(vec![Cell::from("No traces...")]));
        }
    } else {
        ms_rows.push(Row::new(vec![Cell::from("Load Reference Lap".tr(is_ru))]));
    }

    let ms_table = Table::new(
        ms_rows,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .block(ms_block);
    f.render_widget(ms_table, row2[1]);

    let stats_block = Block::default()
        .borders(Borders::ALL)
        .title("Extended Stats".tr(is_ru));
    let stats_rows = vec![
        Row::new(vec![
            Cell::from("Top Speed".tr(is_ru)),
            Cell::from(format!(
                "{:.1} km/h",
                lap.telemetry_trace
                    .iter()
                    .map(|p| p.speed)
                    .fold(0.0, f32::max)
            )),
        ]),
        Row::new(vec![
            Cell::from("Min Speed".tr(is_ru)),
            // Seeded with 999.0 before, so a lap with an empty trace showed
            // "999.0 km/h" as though it were a measurement.
            Cell::from({
                let min_speed = lap
                    .telemetry_trace
                    .iter()
                    .map(|p| p.speed)
                    .fold(f32::INFINITY, f32::min);
                if min_speed.is_finite() {
                    format!("{:.1} km/h", min_speed)
                } else {
                    "—".to_string()
                }
            }),
        ]),
        Row::new(vec![
            Cell::from("Avg Speed".tr(is_ru)),
            Cell::from(format!(
                "{:.1} km/h",
                if !lap.telemetry_trace.is_empty() {
                    lap.telemetry_trace.iter().map(|p| p.speed).sum::<f32>()
                        / lap.telemetry_trace.len() as f32
                } else {
                    0.0
                }
            )),
        ]),
        Row::new(vec![
            Cell::from("Fuel Used".tr(is_ru)),
            Cell::from(format!("{:.2} L", lap.fuel_used)),
        ]),
        Row::new(vec![
            Cell::from("Scrubbing (Errors)".tr(is_ru)),
            Cell::from(format!(
                "{}x (Max {:.0}°)",
                lap.scrubbing_incidents, lap.max_steering_over_rotation
            )),
        ]),
    ];
    f.render_widget(
        Table::new(
            stats_rows,
            [Constraint::Percentage(60), Constraint::Percentage(40)],
        )
        .block(stats_block),
        row2[2],
    );

    let row3 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(chunks[3]);

    let env_block = Block::default()
        .borders(Borders::ALL)
        .title("Environment".tr(is_ru));
    let env_text = vec![
        Line::from(vec![
            Span::raw("Air Temp: "),
            Span::styled(
                fmt.format_temp_prec(lap.air_temp, 1),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("Track Temp: "),
            Span::styled(
                fmt.format_temp_prec(lap.road_temp, 1),
                Style::default().fg(Color::Yellow),
            ),
        ]),
    ];
    f.render_widget(
        Paragraph::new(env_text)
            .block(env_block)
            .alignment(Alignment::Center),
        row3[0],
    );

    let inputs_block = Block::default()
        .borders(Borders::ALL)
        .title("Inputs".tr(is_ru));
    let inputs_text = vec![
        Line::from(vec![
            Span::raw("Throttle Smoothness: "),
            Span::styled(
                format!("{:.1}%", lap.throttle_smoothness),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw("Aggression: "),
            Span::styled(
                format!("{:.1}%", lap.radar_stats.aggression * 100.0),
                Style::default().fg(Color::Red),
            ),
        ]),
    ];
    f.render_widget(
        Paragraph::new(inputs_text)
            .block(inputs_block)
            .alignment(Alignment::Center),
        row3[1],
    );

    let meta_block = Block::default()
        .borders(Borders::ALL)
        .title("Metadata".tr(is_ru));

    let car_name = if !lap.car_model.is_empty() {
        lap.car_model.as_str()
    } else {
        "Unknown".tr(is_ru)
    };

    let track_name = if !lap.track_name.is_empty() {
        lap.track_name.as_str()
    } else {
        "Unknown".tr(is_ru)
    };

    let date_str = if !lap.save_date.is_empty() {
        lap.save_date.as_str()
    } else {
        "--/--/----"
    };

    let meta_text = vec![
        Line::from(vec![
            Span::styled("Car:    ".tr(is_ru), Style::default().fg(Color::Gray)),
            Span::styled(
                car_name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Track:  ".tr(is_ru), Style::default().fg(Color::Gray)),
            Span::styled(
                track_name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Date:   ".tr(is_ru), Style::default().fg(Color::Gray)),
            Span::styled(date_str, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Time:   ".tr(is_ru), Style::default().fg(Color::Gray)),
            Span::styled(&lap.timestamp, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Grip:   ".tr(is_ru), Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.1}%", lap.track_grip),
                Style::default().fg(Color::Green),
            ),
        ]),
    ];

    f.render_widget(
        Paragraph::new(meta_text)
            .block(meta_block)
            .alignment(Alignment::Left),
        row3[2],
    );
}
