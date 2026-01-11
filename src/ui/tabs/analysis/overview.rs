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
        .title(if is_ru {
            "ОБЗОР КРУГА"
        } else {
            "LAP OVERVIEW"
        });

    let min = lap.lap_time_ms / 60000;
    let sec = (lap.lap_time_ms % 60000) / 1000;
    let ms = lap.lap_time_ms % 1000;
    let time_str = format!("{}:{:02}.{:03}", min, sec, ms);

    let diff_text = if let Some(best) = best_lap {
        let diff = lap.lap_time_ms as i32 - best.lap_time_ms as i32;
        let sign = if diff > 0 { "+" } else { "-" };
        let abs_diff = diff.abs();
        let color = if diff > 0 { Color::Red } else { Color::Green };
        Span::styled(
            format!("Delta: {}{}.{:03}", sign, abs_diff / 1000, abs_diff % 1000),
            Style::default().fg(color),
        )
    } else {
        Span::raw("Session Best")
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

    let best_s1 = app
        .analyzer
        .laps
        .iter()
        .filter(|l| l.valid)
        .map(|l| l.sectors[0])
        .min()
        .unwrap_or(0);
    let best_s2 = app
        .analyzer
        .laps
        .iter()
        .filter(|l| l.valid)
        .map(|l| l.sectors[1])
        .min()
        .unwrap_or(0);
    let best_s3 = app
        .analyzer
        .laps
        .iter()
        .filter(|l| l.valid)
        .map(|l| l.sectors[2])
        .min()
        .unwrap_or(0);
    let theoretical_best = best_s1 + best_s2 + best_s3;

    let sec_rows = vec![
        Row::new(vec![
            Cell::from("S1"),
            Cell::from(format!("{:.3}", lap.sectors[0] as f64 / 1000.0)),
            Cell::from(format!("{:.3}", best_s1 as f64 / 1000.0))
                .style(Style::default().fg(Color::Cyan)),
            Cell::from(format!(
                "{:.3}",
                (lap.sectors[0] as i32 - best_s1 as i32) as f64 / 1000.0
            ))
            .style(Style::default().fg(if lap.sectors[0] <= best_s1 {
                Color::Green
            } else {
                Color::Red
            })),
        ]),
        Row::new(vec![
            Cell::from("S2"),
            Cell::from(format!("{:.3}", lap.sectors[1] as f64 / 1000.0)),
            Cell::from(format!("{:.3}", best_s2 as f64 / 1000.0))
                .style(Style::default().fg(Color::Cyan)),
            Cell::from(format!(
                "{:.3}",
                (lap.sectors[1] as i32 - best_s2 as i32) as f64 / 1000.0
            ))
            .style(Style::default().fg(if lap.sectors[1] <= best_s2 {
                Color::Green
            } else {
                Color::Red
            })),
        ]),
        Row::new(vec![
            Cell::from("S3"),
            Cell::from(format!("{:.3}", lap.sectors[2] as f64 / 1000.0)),
            Cell::from(format!("{:.3}", best_s3 as f64 / 1000.0))
                .style(Style::default().fg(Color::Cyan)),
            Cell::from(format!(
                "{:.3}",
                (lap.sectors[2] as i32 - best_s3 as i32) as f64 / 1000.0
            ))
            .style(Style::default().fg(if lap.sectors[2] <= best_s3 {
                Color::Green
            } else {
                Color::Red
            })),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Теор. Оптим."
            } else {
                "Optimal"
            }),
            Cell::from("----"),
            Cell::from(format!("{:.3}", theoretical_best as f64 / 1000.0))
                .style(Style::default().fg(Color::Magenta)),
            Cell::from(format!(
                "{:.3}",
                (lap.lap_time_ms as i32 - theoretical_best as i32) as f64 / 1000.0
            ))
            .style(Style::default().fg(Color::Yellow)),
        ]),
    ];

    let sec_table = Table::new(sec_rows, [Constraint::Ratio(1, 4); 4])
        .header(
            Row::new(vec!["Sec", "Time", "Best", "Diff"]).style(Style::default().fg(Color::Gray)),
        )
        .block(Block::default().borders(Borders::ALL).title(if is_ru {
            "Сектора"
        } else {
            "Sector Analysis"
        }));
    f.render_widget(sec_table, row1[0]);

    let score_block = Block::default().borders(Borders::ALL).title(if is_ru {
        "Оценка Вождения"
    } else {
        "Driving Evaluation"
    });
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
            .ratio(val / 100.0)
            .label(format!("{}: {:.0}/100", label, val))
    };

    f.render_widget(
        make_gauge(
            if is_ru {
                "Общий Рейтинг"
            } else {
                "Overall Score"
            },
            overall_score,
            Color::Magenta,
        ),
        score_layout[0],
    );
    f.render_widget(
        make_gauge(
            if is_ru {
                "Стабильность"
            } else {
                "Stability"
            },
            stability_score,
            Color::Green,
        ),
        score_layout[1],
    );
    f.render_widget(
        make_gauge(
            if is_ru {
                "Агрессия"
            } else {
                "Aggression"
            },
            aggression_score,
            Color::Red,
        ),
        score_layout[2],
    );
    f.render_widget(
        make_gauge(
            if is_ru {
                "Использ. Грипа"
            } else {
                "Grip Usage"
            },
            grip_score,
            Color::Cyan,
        ),
        score_layout[3],
    );

    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    let car_schema_block = Block::default().borders(Borders::ALL).title(if is_ru {
        "Схема: Колеса и Температуры"
    } else {
        "Car Schema: Wheels & Temps"
    });
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
        let wear = 100.0;

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
                Span::raw("Tyre: "),
                Span::styled(format!("{:.0}C", temp), Style::default().fg(color)),
            ]),
            Line::from(vec![
                Span::raw("Brake: "),
                Span::styled(
                    format!("{:.0}C", brake),
                    Style::default().fg(if brake > 600.0 {
                        Color::Red
                    } else {
                        Color::Yellow
                    }),
                ),
            ]),
            Line::from(vec![
                Span::raw("Wear: "),
                Span::styled(format!("{:.0}%", wear), Style::default().fg(Color::Green)),
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

    let stats_block = Block::default().borders(Borders::ALL).title(if is_ru {
        "Расширенная Статистика"
    } else {
        "Extended Stats"
    });
    let stats_rows = vec![
        Row::new(vec![
            Cell::from(if is_ru {
                "Макс. Скорость"
            } else {
                "Top Speed"
            }),
            Cell::from(format!(
                "{:.1} km/h",
                lap.telemetry_trace
                    .iter()
                    .map(|p| p.speed)
                    .fold(0.0, f32::max)
            )),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Мин. Скорость"
            } else {
                "Min Speed"
            }),
            Cell::from(format!(
                "{:.1} km/h",
                lap.telemetry_trace
                    .iter()
                    .map(|p| p.speed)
                    .fold(999.0, f32::min)
            )),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Средняя Скорость"
            } else {
                "Avg Speed"
            }),
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
            Cell::from(if is_ru {
                "Переключения"
            } else {
                "Gear Shifts"
            }),
            Cell::from(format!("{}", lap.gear_shifts)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Расход Топлива"
            } else {
                "Fuel Used"
            }),
            Cell::from(format!("{:.2} L", lap.fuel_used)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Ошибки (Lockups)"
            } else {
                "Lockups"
            }),
            Cell::from(format!("{}", lap.lockup_count)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Ошибки (Spin)"
            } else {
                "Spins/Slides"
            }),
            Cell::from(format!("{}", lap.oversteer_count)),
        ]),
    ];
    f.render_widget(
        Table::new(
            stats_rows,
            [Constraint::Percentage(60), Constraint::Percentage(40)],
        )
        .block(stats_block),
        row2[1],
    );

    let row3 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(chunks[3]);

    let env_block = Block::default().borders(Borders::ALL).title(if is_ru {
        "Среда"
    } else {
        "Environment"
    });
    let env_text = vec![
        Line::from(vec![
            Span::raw("Air Temp: "),
            Span::styled(
                format!("{:.1} C", lap.air_temp),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("Track Temp: "),
            Span::styled(
                format!("{:.1} C", lap.road_temp),
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

    let inputs_block =
        Block::default()
            .borders(Borders::ALL)
            .title(if is_ru { "Ввод" } else { "Inputs" });
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

    let meta_block = Block::default().borders(Borders::ALL).title(if is_ru {
        "Метаданные"
    } else {
        "Metadata"
    });

    let car_name = if !lap.car_model.is_empty() {
        lap.car_model.as_str()
    } else {
        if is_ru {
            "Неизвестно"
        } else {
            "Unknown"
        }
    };

    let track_name = if !lap.track_name.is_empty() {
        lap.track_name.as_str()
    } else {
        if is_ru {
            "Неизвестно"
        } else {
            "Unknown"
        }
    };

    let date_str = if !lap.save_date.is_empty() {
        lap.save_date.as_str()
    } else {
        "--/--/----"
    };

    let meta_text = vec![
        Line::from(vec![
            Span::styled(
                if is_ru { "Авто:   " } else { "Car:    " },
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                car_name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_ru { "Трасса: " } else { "Track:  " },
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                track_name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if is_ru { "Дата:   " } else { "Date:   " },
                Style::default().fg(Color::Gray),
            ),
            Span::styled(date_str, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(
                if is_ru { "Время:  " } else { "Time:   " },
                Style::default().fg(Color::Gray),
            ),
            Span::styled(&lap.timestamp, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(
                if is_ru { "Грип:   " } else { "Grip:   " },
                Style::default().fg(Color::Gray),
            ),
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
