use crate::AppState;
use ratatui::widgets::canvas::{Canvas, Circle, Line as CanvasLine, Points};
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState, lap: &ac_core::analyzer::LapData) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let left_col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[0]);

    let temp_block = Block::default()
        .title(if is_ru {
            "Температуры (C)"
        } else {
            "Temps (C)"
        })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner_temp = temp_block.inner(left_col[0]);
    f.render_widget(temp_block, left_col[0]);

    let wheel_names = ["FL", "FR", "RL", "RR"];
    let wheel_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 4); 4])
        .split(inner_temp);

    for i in 0..4 {
        let t_tyre = *lap.avg_tyre_temp.get(i).unwrap_or(&0.0);
        let t_brake = *lap.max_brake_temp.get(i).unwrap_or(&0.0);

        let row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(3),
                Constraint::Ratio(1, 2),
                Constraint::Ratio(1, 2),
            ])
            .split(wheel_rows[i]);
        f.render_widget(
            Paragraph::new(wheel_names[i]).style(Style::default().add_modifier(Modifier::BOLD)),
            row[0],
        );

        let t_color = if t_tyre > 100.0 {
            Color::Red
        } else if t_tyre < 70.0 {
            Color::Blue
        } else {
            Color::Green
        };
        f.render_widget(
            Gauge::default()
                .ratio((t_tyre / 150.0).clamp(0.0, 1.0) as f64)
                .gauge_style(Style::default().fg(t_color))
                .label(format!("T:{:.0}", t_tyre)),
            row[1],
        );

        let b_color = if t_brake > 600.0 {
            Color::Red
        } else {
            Color::Yellow
        };
        f.render_widget(
            Gauge::default()
                .ratio((t_brake / 1000.0).clamp(0.0, 1.0) as f64)
                .gauge_style(Style::default().fg(b_color))
                .label(format!("B:{:.0}", t_brake)),
            row[2],
        );
    }

    let susp_block = Block::default()
        .title(if is_ru {
            "Амортизаторы (Сжатие/Отбой)"
        } else {
            "Damper Histograms (Bump/Rebound)"
        })
        .borders(Borders::ALL);
    let inner_susp = susp_block.inner(left_col[1]);
    f.render_widget(susp_block, left_col[1]);

    let susp_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 4); 4])
        .split(inner_susp);

    for i in 0..4 {
        let hist = lap.damper_histograms.get(i).unwrap_or(&[0.0; 4]);

        let fb_bars = "█".repeat((hist[1] / 3.0) as usize);
        let sb_bars = "█".repeat((hist[0] / 3.0) as usize);
        let sr_bars = "█".repeat((hist[2] / 3.0) as usize);
        let fr_bars = "█".repeat((hist[3] / 3.0) as usize);

        let line1 = Line::from(vec![
            Span::styled(
                format!("{:<2} Bump: ", wheel_names[i]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("FB {:>2.0}% {} ", hist[1], fb_bars),
                Style::default().fg(Color::LightRed),
            ),
            Span::styled(
                format!("SB {:>2.0}% {}", hist[0], sb_bars),
                Style::default().fg(Color::Yellow),
            ),
        ]);
        let line2 = Line::from(vec![
            Span::styled("   Reb:  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("FR {:>2.0}% {} ", hist[3], fr_bars),
                Style::default().fg(Color::LightBlue),
            ),
            Span::styled(
                format!("SR {:>2.0}% {}", hist[2], sr_bars),
                Style::default().fg(Color::Cyan),
            ),
        ]);

        f.render_widget(Paragraph::new(vec![line1, line2]), susp_rows[i]);
    }

    let right_col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(layout[1]);

    let mut total_g = 0.0;
    let mut count = 0;
    let mut max_g = 0.0;

    for p in &lap.telemetry_trace {
        if p.speed > 20.0 {
            let g = (p.lat_g.powi(2) + p.lon_g.powi(2)).sqrt();
            total_g += g;
            if g > max_g {
                max_g = g;
            }
            count += 1;
        }
    }
    let avg_g = if count > 0 {
        total_g / count as f32
    } else {
        0.0
    };
    let grip_usage = (avg_g / 2.5 * 100.0).clamp(0.0, 100.0);

    let gg_title = if is_ru {
        format!("G-G (Исп. сцепления: {:.0}%)", grip_usage)
    } else {
        format!("G-G Plot (Grip Usage: {:.0}%)", grip_usage)
    };

    let gg_block = Block::default()
        .title(gg_title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let canvas = Canvas::default()
        .block(gg_block)
        .x_bounds([-3.5, 3.5])
        .y_bounds([-3.5, 3.5])
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
                radius: 2.5,
                color: Color::Gray,
            });

            ctx.draw(&CanvasLine {
                x1: -3.5,
                y1: 0.0,
                x2: 3.5,
                y2: 0.0,
                color: Color::DarkGray,
            });
            ctx.draw(&CanvasLine {
                x1: 0.0,
                y1: -3.5,
                x2: 0.0,
                y2: 3.5,
                color: Color::DarkGray,
            });

            for p in &lap.telemetry_trace {
                if p.speed > 10.0 {
                    let color = if p.brake > 0.1 {
                        Color::Red
                    } else if p.gas > 0.1 {
                        Color::Green
                    } else {
                        Color::Yellow
                    };
                    ctx.draw(&Points {
                        coords: &[(p.lat_g as f64, p.lon_g as f64)],
                        color,
                    });
                }
            }
        });
    f.render_widget(canvas, right_col[0]);

    let stab_block = Block::default()
        .title(if is_ru {
            "Стабильность"
        } else {
            "Stability"
        })
        .borders(Borders::ALL);

    let stab_data = vec![
        Row::new(vec![
            Cell::from(if is_ru {
                "Снос передней (Under)"
            } else {
                "Understeer"
            }),
            Cell::from(lap.understeer_count.to_string()).style(Style::default().fg(Color::Yellow)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Занос задней (Over)"
            } else {
                "Oversteer"
            }),
            Cell::from(lap.oversteer_count.to_string()).style(Style::default().fg(Color::Red)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru {
                "Блокировки (Lockup)"
            } else {
                "Lockups"
            }),
            Cell::from(lap.lockup_count.to_string()).style(Style::default().fg(Color::Magenta)),
        ]),
        Row::new(vec![
            Cell::from(if is_ru { "Пик G-Force" } else { "Peak G" }),
            Cell::from(format!("{:.2} G", max_g)).style(Style::default().fg(Color::Cyan)),
        ]),
    ];
    f.render_widget(
        Table::new(
            stab_data,
            [Constraint::Percentage(70), Constraint::Percentage(30)],
        )
        .block(stab_block),
        right_col[1],
    );
}
