use crate::ui::localization::tr;
use crate::AppState;
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;

    if app.analyzer.laps.is_empty() {
        let block = Block::default()
            .title(tr("tab_anal", lang))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        let text = Paragraph::new(tr("anal_waiting", lang))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(text, area);
        return;
    }

    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(area);

    render_laps_list(f, main_layout[0], app);

    let selected_idx = app.ui_state.setup_list_state.selected().unwrap_or(0);
    if selected_idx < app.analyzer.laps.len() {
        let selected_lap = &app.analyzer.laps[selected_idx];
        let best_lap = app.analyzer.best_lap_index.map(|i| &app.analyzer.laps[i]);

        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Min(10),
                Constraint::Length(12),
            ])
            .split(main_layout[1]);

        render_header_stats(f, right_layout[0], app, selected_lap);
        render_speed_chart(f, right_layout[1], app, selected_lap, best_lap);

        let bottom_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(40),
            ])
            .split(right_layout[2]);

        render_radar_chart(f, bottom_layout[0], app, selected_lap);
        render_extended_stats(f, bottom_layout[1], app, selected_lap);
        render_coach_report(f, bottom_layout[2], app, selected_lap);
    }
}

fn render_laps_list(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("anal_laps_list", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    let items: Vec<ListItem<'_>> = app
        .analyzer
        .laps
        .iter()
        .enumerate()
        .map(|(i, lap)| {
            let is_best = Some(i) == app.analyzer.best_lap_index;
            let prefix = if is_best { "★" } else { " " };
            let mut style = Style::default().fg(app.ui_state.get_color(&theme.text));
            if is_best {
                style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
            }
            ListItem::new(format!(
                "{} L{}: {}",
                prefix,
                lap.lap_number + 1,
                format_ms(lap.lap_time_ms)
            ))
            .style(style)
        })
        .collect();
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(app.ui_state.get_color(&theme.highlight))
            .fg(Color::Black),
    );
    let mut state = app.ui_state.setup_list_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_header_stats(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap: &crate::analyzer::LapData,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("anal_session_info", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(inner);

    let wr_time = app
        .analyzer
        .world_record
        .as_ref()
        .map(|r| r.time_ms)
        .unwrap_or(0);
    let wr_delta = if wr_time > 0 {
        (lap.lap_time_ms - wr_time) as f32 / 1000.0
    } else {
        0.0
    };
    let wr_color = if wr_delta <= 0.0 {
        Color::Green
    } else {
        Color::Red
    };

    let times_rows = vec![
        Row::new(vec![
            Cell::from(tr("anal_time", lang)),
            Cell::from(format_ms(lap.lap_time_ms)).style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Row::new(vec![
            Cell::from(tr("anal_wr", lang)),
            Cell::from(if wr_time > 0 {
                format_ms(wr_time)
            } else {
                "---".into()
            })
            .style(Style::default().fg(Color::Yellow)),
        ]),
        Row::new(vec![
            Cell::from(tr("anal_delta", lang)),
            Cell::from(format!("{:+.3}s", wr_delta)).style(Style::default().fg(wr_color)),
        ]),
    ];
    f.render_widget(
        Table::new(
            times_rows,
            [Constraint::Percentage(40), Constraint::Percentage(60)],
        ),
        chunks[0],
    );

    let info_rows = vec![
        Row::new(vec![
            Cell::from(tr("info_car", lang)).style(Style::default().fg(Color::Gray)),
            Cell::from(app.session_info.car_name.clone()).style(Style::default().fg(Color::Cyan)),
            Cell::from(tr("info_track", lang)).style(Style::default().fg(Color::Gray)),
            Cell::from(app.session_info.track_name.clone()).style(Style::default().fg(Color::Cyan)),
        ]),
        Row::new(vec![
            Cell::from(tr("info_cond", lang)).style(Style::default().fg(Color::Gray)),
            Cell::from(format!(
                "Air: {:.0}°C  Road: {:.0}°C",
                lap.air_temp, lap.road_temp
            ))
            .style(Style::default().fg(Color::White)),
            Cell::from(tr("info_grip", lang)).style(Style::default().fg(Color::Gray)),
            Cell::from(format!("{:.1}%", lap.track_grip)).style(Style::default().fg(Color::Green)),
        ]),
    ];
    f.render_widget(
        Table::new(
            info_rows,
            [
                Constraint::Percentage(15),
                Constraint::Percentage(35),
                Constraint::Percentage(15),
                Constraint::Percentage(35),
            ],
        ),
        chunks[1],
    );
}

fn render_speed_chart(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    selected: &crate::analyzer::LapData,
    best: Option<&crate::analyzer::LapData>,
) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    let selected_data: Vec<(f64, f64)> = selected
        .telemetry_trace
        .iter()
        .map(|p| (p.distance as f64, p.speed as f64))
        .collect();

    let datasets = vec![Dataset::default()
        .name("Lap")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .graph_type(GraphType::Line)
        .data(&selected_data)];

    if let Some(best_l) = best {
        if selected.lap_number != best_l.lap_number {}
    }
    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(tr("anal_speed_comp", lang))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
        )
        .x_axis(Axis::default().bounds([0.0, 1.0]).labels(vec![]))
        .y_axis(Axis::default().bounds([0.0, 320.0]));
    f.render_widget(chart, area);
}

fn render_radar_chart(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap: &crate::analyzer::LapData,
) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    let stats = &lap.radar_stats;
    let values = [
        stats.smoothness,
        stats.aggression,
        stats.consistency,
        stats.car_control,
        stats.tyre_mgmt,
    ];
    let labels = [
        tr("skill_smooth", lang),
        tr("skill_aggr", lang),
        tr("skill_consist", lang),
        tr("skill_car_ctrl", lang),
        tr("skill_tyres", lang),
    ];

    let canvas = Canvas::default()
        .block(
            Block::default()
                .title(tr("anal_radar_title", lang))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
        )
        .x_bounds([-1.5, 1.5])
        .y_bounds([-1.5, 1.5])
        .paint(move |ctx| {
            let radius = 1.0;
            let count = 5;
            for i in 0..count {
                let angle = (i as f64) * 2.0 * std::f64::consts::PI / (count as f64)
                    - std::f64::consts::PI / 2.0;
                let x = radius * angle.cos();
                let y = radius * angle.sin();
                ctx.draw(&CanvasLine {
                    x1: 0.0,
                    y1: 0.0,
                    x2: x,
                    y2: y,
                    color: Color::DarkGray,
                });
                ctx.print(
                    x * 1.2 - 0.2,
                    y * 1.2,
                    Span::styled(labels[i].clone(), Style::default().fg(Color::Gray)),
                );
            }
            for i in 0..count {
                let val1 = values[i] as f64;
                let val2 = values[(i + 1) % count] as f64;
                let angle1 = (i as f64) * 2.0 * std::f64::consts::PI / (count as f64)
                    - std::f64::consts::PI / 2.0;
                let angle2 = ((i + 1) as f64) * 2.0 * std::f64::consts::PI / (count as f64)
                    - std::f64::consts::PI / 2.0;
                ctx.draw(&CanvasLine {
                    x1: radius * val1 * angle1.cos(),
                    y1: radius * val1 * angle1.sin(),
                    x2: radius * val2 * angle2.cos(),
                    y2: radius * val2 * angle2.sin(),
                    color: Color::Cyan,
                });
            }
        });
    f.render_widget(canvas, area);
}

fn render_extended_stats(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    selected: &crate::analyzer::LapData,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("anal_stats_ext", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let params = vec![
        (
            tr("anal_max_spd", lang),
            format!("{:.0}", selected.max_speed),
            Color::White,
        ),
        (
            tr("anal_g_lat", lang),
            format!("{:.2}G", selected.peak_lat_g),
            Color::Magenta,
        ),
        (
            tr("anal_p_dev", lang),
            format!("{:.2} psi", selected.pressure_deviation),
            if selected.pressure_deviation > 1.0 {
                Color::Red
            } else {
                Color::Green
            },
        ),
        (
            tr("anal_avg_spd", lang),
            format!("{:.0}", selected.avg_speed),
            Color::Gray,
        ),
    ];
    let items: Vec<ListItem<'_>> = params
        .into_iter()
        .map(|(l, v, c)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}: ", l), Style::default().fg(Color::Gray)),
                Span::styled(v, Style::default().fg(c)),
            ]))
        })
        .collect();
    f.render_widget(List::new(items), inner);
}

fn render_coach_report(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    selected: &crate::analyzer::LapData,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == crate::config::Language::Russian;
    let block = Block::default()
        .title(tr("anal_comp_title", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let analysis = app.analyzer.analyze_standalone(selected, lang);

    if analysis.is_perfect {
        f.render_widget(
            Paragraph::new(tr("anal_self_perfect", lang))
                .style(Style::default().fg(Color::Green))
                .alignment(Alignment::Center),
            inner,
        );
    } else {
        let items: Vec<ListItem<'_>> = analysis
            .advices
            .iter()
            .map(|a| {
                let color = match a.severity {
                    3 => Color::Red,
                    2 => Color::Yellow,
                    _ => Color::Blue,
                };
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!("⚠ [{}]: ", a.zone),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(&a.problem),
                    ]),
                    Line::from(vec![
                        Span::raw(if is_ru { "   Совет: " } else { "   Fix: " }),
                        Span::styled(&a.solution, Style::default().fg(Color::Green)),
                    ]),
                    Line::from(""),
                ])
            })
            .collect();
        f.render_widget(List::new(items), inner);
    }
}

fn format_ms(ms: i32) -> String {
    format!("{}:{:02}.{:03}", ms / 60000, (ms % 60000) / 1000, ms % 1000)
}
