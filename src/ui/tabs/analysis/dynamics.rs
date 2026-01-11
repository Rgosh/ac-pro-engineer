use crate::AppState;
use ratatui::widgets::canvas::{Canvas, Circle, Line as CanvasLine, Points};
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState, lap: &crate::analyzer::LapData) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == crate::config::Language::Russian;

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let left_col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
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
            "Работа Подвески"
        } else {
            "Suspension Work"
        })
        .borders(Borders::ALL);
    let inner_susp = susp_block.inner(left_col[1]);
    f.render_widget(susp_block, left_col[1]);

    let susp_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 4); 4])
        .split(inner_susp);
    for i in 0..4 {
        let travel = *lap.suspension_travel_hist.get(i).unwrap_or(&0.0) * 1000.0;
        let label = format!("{}: {:.1}mm avg", wheel_names[i], travel);
        let bars = (travel / 5.0).clamp(0.0, 20.0) as usize;
        let bar_str = "█".repeat(bars);
        f.render_widget(
            Paragraph::new(format!("{} {}", label, bar_str))
                .style(Style::default().fg(Color::Cyan)),
            susp_rows[i],
        );
    }

    let right_col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(layout[1]);

    let gg_block = Block::default()
        .title("G-G Plot")
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
