use crate::ui::localization::tr;
use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;

    let (gfx, phys) = if let (Some(g_mem), Some(p_mem)) = (&app.graphics_mem, &app.physics_mem) {
        (g_mem.get(), p_mem.get())
    } else {
        let block = Block::default()
            .title(tr("tab_strat", lang))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        let text = Paragraph::new(tr("no_data", lang))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(text, area);
        return;
    };

    let v_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(v_layout[0]);

    render_fuel_calculator(f, top_layout[0], app, gfx, phys);

    let top_right_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(top_layout[1]);

    render_tyres_strategy(f, top_right_layout[0], app, phys);
    render_environment(f, top_right_layout[1], app, gfx, phys);

    render_pace_history(f, v_layout[1], app);
}

fn render_pace_history(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == crate::config::Language::Russian;

    let block = Block::default()
        .title(if is_ru {
            "История Темпа (Stint Pace)"
        } else {
            "Race Pace History"
        })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    if app.analyzer.laps.is_empty() {
        let p = Paragraph::new(if is_ru {
            "Нет завершенных кругов"
        } else {
            "No completed laps yet"
        })
        .block(block)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    }

    let laps: Vec<(f64, f64)> = app
        .analyzer
        .laps
        .iter()
        .map(|l| (l.lap_number as f64, l.lap_time_ms as f64 / 1000.0))
        .collect();

    let min_time = laps
        .iter()
        .map(|(_, t)| *t)
        .fold(f64::INFINITY, |a, b| a.min(b));
    let max_time = laps.iter().map(|(_, t)| *t).fold(0.0f64, |a, b| a.max(b));

    let y_min = (min_time - 1.0).max(0.0);
    let y_max = max_time + 1.0;

    let x_max = laps.last().map(|(n, _)| *n).unwrap_or(10.0) + 1.0;
    let x_min = laps.first().map(|(n, _)| *n).unwrap_or(0.0);

    let datasets = vec![Dataset::default()
        .name(if is_ru {
            "Время круга"
        } else {
            "Lap Time"
        })
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .graph_type(GraphType::Line)
        .data(&laps)];

    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .title("Lap")
                .style(Style::default().fg(Color::Gray))
                .bounds([x_min, x_max])
                .labels(vec![
                    Span::from(format!("{:.0}", x_min)),
                    Span::from(format!("{:.0}", x_max)),
                ]),
        )
        .y_axis(
            Axis::default()
                .title("Sec")
                .style(Style::default().fg(Color::Gray))
                .bounds([y_min, y_max])
                .labels(vec![
                    Span::from(format!("{:.1}", y_min)),
                    Span::from(format!("{:.1}", y_max)),
                ]),
        );

    f.render_widget(chart, area);
}

fn render_fuel_calculator(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    gfx: &crate::ac_structs::AcGraphics,
    phys: &crate::ac_structs::AcPhysics,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == crate::config::Language::Russian;

    let block = Block::default()
        .title(tr("strat_fuel_title", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let fuel_per_lap = gfx.fuel_x_lap;
    let current_fuel = phys.fuel;

    let mut laps_remaining = 0.0;

    if gfx.number_of_laps > 0 {
        laps_remaining =
            (gfx.number_of_laps as f32 - gfx.completed_laps as f32 - gfx.normalized_car_position)
                .max(0.0);
    } else if gfx.session_time_left > 0.0 {
        let lap_time_ms = if gfx.i_best_time > 0 {
            gfx.i_best_time
        } else if gfx.i_last_time > 0 {
            gfx.i_last_time
        } else {
            120000
        };
        let lap_time_sec = lap_time_ms as f32 / 1000.0;

        if lap_time_sec > 0.0 {
            laps_remaining = gfx.session_time_left / 1000.0 / lap_time_sec;
        }
    }

    if laps_remaining == 0.0 && gfx.session < 3 {
        laps_remaining = 5.0;
    }

    let fuel_needed = laps_remaining * fuel_per_lap;
    let safety_margin = 1.0 * fuel_per_lap;
    let total_needed_safe = fuel_needed + safety_margin;

    let fuel_delta = current_fuel - total_needed_safe;

    let (verdict_text, verdict_color, sub_verdict) = if fuel_per_lap <= 0.0 {
        (
            if is_ru {
                "НЕТ ДАННЫХ"
            } else {
                "NO DATA"
            },
            Color::Gray,
            if is_ru {
                "Проедьте пару кругов..."
            } else {
                "Drive more laps..."
            },
        )
    } else if fuel_delta >= 0.0 {
        (
            if is_ru {
                "ТОПЛИВА ХВАТАЕТ"
            } else {
                "FUEL IS SAFE"
            },
            Color::Green,
            if is_ru {
                "Дозаправка не требуется"
            } else {
                "No refueling needed"
            },
        )
    } else {
        (
            if is_ru {
                "НУЖЕН ПИТ-СТОП"
            } else {
                "REFUEL NEEDED"
            },
            Color::Red,
            if is_ru {
                "Не хватит до финиша"
            } else {
                "Not enough to finish"
            },
        )
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let verdict_p = Paragraph::new(vec![
        Line::from(Span::styled(
            verdict_text,
            Style::default()
                .fg(verdict_color)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(Span::styled(sub_verdict, Style::default().fg(Color::White))),
    ])
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(verdict_p, layout[0]);

    let rows = vec![
        Row::new(vec![
            Cell::from(tr("strat_cons", lang)),
            Cell::from(format!("{:.2} L/lap", fuel_per_lap))
                .style(Style::default().fg(Color::Yellow)),
        ]),
        Row::new(vec![
            Cell::from(tr("strat_laps_rem", lang)),
            Cell::from(format!("{:.1} laps", laps_remaining))
                .style(Style::default().fg(Color::Cyan)),
        ]),
        Row::new(vec![
            Cell::from(tr("strat_fuel_rem", lang)),
            Cell::from(format!("{:.1} L", current_fuel))
                .style(Style::default().fg(get_fuel_color(app.engineer.stats.fuel_laps_remaining))),
        ]),
        Row::new(vec![
            Cell::from(tr("strat_needed", lang)),
            Cell::from(format!("{:.1} L", total_needed_safe))
                .style(Style::default().fg(Color::White)),
        ]),
        Row::new(vec![
            Cell::from(tr("strat_delta", lang)),
            Cell::from(format!("{:.1} L", fuel_delta)).style(
                Style::default()
                    .fg(if fuel_delta >= 0.0 {
                        Color::Green
                    } else {
                        Color::Red
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let table = Table::new(
        rows,
        [Constraint::Percentage(60), Constraint::Percentage(40)],
    )
    .block(Block::default().padding(Padding::new(1, 1, 1, 0)));
    f.render_widget(table, layout[2]);
}

fn render_tyres_strategy(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    phys: &crate::ac_structs::AcPhysics,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;

    let block = Block::default()
        .title(tr("strat_tyres_title", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let tyre_names = ["FL", "FR", "RL", "RR"];

    for i in 0..4 {
        if i >= layout.len() {
            break;
        }

        let wear = phys.tyre_wear[i];
        let health_pct = ((wear - 94.0) / 6.0 * 100.0).clamp(0.0, 100.0);

        let color = if wear > 98.0 {
            Color::Green
        } else if wear > 96.0 {
            Color::Yellow
        } else {
            Color::Red
        };

        let label = format!("{} ({:.1}%)", tyre_names[i], wear);

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
            .ratio(health_pct as f64 / 100.0)
            .label(label);

        f.render_widget(gauge, layout[i]);
    }
}

fn render_environment(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    gfx: &crate::ac_structs::AcGraphics,
    phys: &crate::ac_structs::AcPhysics,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;

    let block = Block::default()
        .title(tr("strat_env_title", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = vec![
        Row::new(vec![
            Cell::from(tr("strat_grip", lang)),
            Cell::from(format!("{:.1}%", gfx.surface_grip * 100.0)).style(Style::default().fg(
                if gfx.surface_grip > 0.95 {
                    Color::Green
                } else {
                    Color::Red
                },
            )),
        ]),
        Row::new(vec![
            Cell::from(tr("strat_air", lang)),
            Cell::from(format!("{:.1}°C", phys.air_temp)).style(Style::default().fg(Color::Cyan)),
        ]),
        Row::new(vec![
            Cell::from(tr("strat_road", lang)),
            Cell::from(format!("{:.1}°C", phys.road_temp))
                .style(Style::default().fg(Color::Yellow)),
        ]),
        Row::new(vec![
            Cell::from(tr("strat_wind", lang)),
            Cell::from(format!("{:.1} km/h", gfx.wind_speed))
                .style(Style::default().fg(Color::White)),
        ]),
    ];

    let table = Table::new(
        rows,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    );
    f.render_widget(table, inner);
}

fn get_fuel_color(laps: f32) -> Color {
    if laps > 3.0 {
        Color::Green
    } else if laps > 1.5 {
        Color::Yellow
    } else {
        Color::Red
    }
}
