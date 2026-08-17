use crate::AppState;
use ac_core::i18n::{Translate, tr_fmt};
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;

    // Through the accessors, so --demo populates this tab like the others.
    // Reading `app.mem` directly meant the whole strategy tab said "no data"
    // in demo mode.
    let (Some(gfx_ref), Some(phys_ref)) = (app.session(), app.car()) else {
        let block = Block::default()
            .title("STRATEGY".tr_lang(lang).to_string())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        let message = if app.is_game_running {
            "Waiting for data...".tr_lang(lang).to_string()
        } else {
            // The chosen game, not a hardcoded one. See the dashboard's
            // waiting panel for what this said before.
            tr_fmt(
                "{0} is not running",
                *lang == ac_core::config::Language::Russian,
                &[app.game.name],
            )
        };
        let text = Paragraph::new(message)
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(text, area);
        return;
    };

    let gfx = *gfx_ref;
    let phys = *phys_ref;

    let v_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(v_layout[0]);

    render_fuel_calculator(f, top_layout[0], app, &gfx, &phys);

    let top_right_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(top_layout[1]);

    render_tyres_strategy(f, top_right_layout[0], app, &phys);
    render_environment(f, top_right_layout[1], app, &gfx, &phys);

    render_pace_history(f, v_layout[1], app);
}

fn render_pace_history(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let block = Block::default()
        .title("Race Pace History".tr(is_ru))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    if app.analyzer.laps.is_empty() {
        let p = Paragraph::new("No completed laps yet".tr(is_ru))
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

    let datasets = vec![
        Dataset::default()
            .name("Lap Time".tr(is_ru))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .graph_type(GraphType::Line)
            .data(&laps),
    ];

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
    gfx: &ac_core::games::Session,
    phys: &ac_core::games::Car,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == ac_core::config::Language::Russian;

    let block = Block::default()
        .title("Fuel Calculator".tr_lang(lang).to_string())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let fuel_per_lap = gfx.fuel_per_lap;
    let current_fuel = phys.fuel_litres;

    let mut laps_remaining = ac_core::session_info::SessionTiming::remaining_laps(
        gfx.session_time_left_ms,
        gfx.best_lap_ms,
        gfx.last_lap_ms,
        gfx.total_laps,
        gfx.completed_laps,
        gfx.track_position,
    );

    if laps_remaining == 0.0 && gfx.kind.has_no_finish() {
        laps_remaining = 5.0;
    }

    let fuel_needed = laps_remaining * fuel_per_lap;
    let safety_margin = 1.0 * fuel_per_lap;
    let total_needed_safe = fuel_needed + safety_margin;

    let fuel_delta = current_fuel - total_needed_safe;

    let (verdict_text, verdict_color, sub_verdict) = if fuel_per_lap <= 0.0 {
        (
            "NO DATA".tr(is_ru),
            Color::Gray,
            "Drive more laps...".tr(is_ru),
        )
    } else if fuel_delta >= 0.0 {
        (
            "FUEL IS SAFE".tr(is_ru),
            Color::Green,
            "No refueling needed".tr(is_ru),
        )
    } else {
        (
            "REFUEL NEEDED".tr(is_ru),
            Color::Red,
            "Not enough to finish".tr(is_ru),
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
            Cell::from("Avg Cons.".tr_lang(lang).to_string()),
            Cell::from(format!("{:.2} L/lap", fuel_per_lap))
                .style(Style::default().fg(Color::Yellow)),
        ]),
        Row::new(vec![
            Cell::from("Laps Rem.".tr_lang(lang).to_string()),
            Cell::from(format!("{:.1} laps", laps_remaining))
                .style(Style::default().fg(Color::Cyan)),
        ]),
        Row::new(vec![
            Cell::from("Fuel Rem.".tr_lang(lang).to_string()),
            Cell::from(format!("{:.1} L", current_fuel))
                .style(Style::default().fg(get_fuel_color(app.engineer.stats.fuel_laps_remaining))),
        ]),
        Row::new(vec![
            Cell::from("Fuel Needed".tr_lang(lang).to_string()),
            Cell::from(format!("{:.1} L", total_needed_safe))
                .style(Style::default().fg(Color::White)),
        ]),
        Row::new(vec![
            Cell::from("Fuel Delta".tr_lang(lang).to_string()),
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
    phys: &ac_core::games::Car,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == ac_core::config::Language::Russian;

    // What this panel projects depends on what the game measures. Competizione
    // publishes no tyre wear, so all four rows read "0.0%" in red and the bars
    // drew empty — a set with nothing left, for a game that never said. It
    // publishes what is left of the brake pads instead, which over a GT3 stint
    // is the consumable that actually decides the race.
    let measures = app
        .reading
        .as_ref()
        .map(|reading| reading.capabilities)
        .unwrap_or_else(ac_core::games::Capabilities::all);
    let brakes_instead = !measures.tyre_wear && measures.brake_wear;

    let block = Block::default()
        .title(if brakes_instead {
            "Brake Life".tr(is_ru)
        } else {
            "Tyre Life Predictor".tr(is_ru)
        })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Neither measured: say so once rather than drawing four zeros.
    if !measures.tyre_wear && !measures.brake_wear {
        f.render_widget(
            Paragraph::new("This game does not report wear".tr(is_ru).to_string())
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().padding(Padding::new(1, 1, 1, 0))),
            inner,
        );
        return;
    }

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
    let critical = app.config.alerts.wear_critical.clamp(0.0, 99.0);
    let warning = app.config.alerts.wear_warning.clamp(critical, 100.0);

    for (i, name) in tyre_names.iter().enumerate() {
        let Some(row) = layout.get(i) else {
            break;
        };

        // On a game that measures the pads and not the tyres, the same four
        // rows carry the same shape of number: how much of the consumable is
        // left, scaled between "finished" and "new".
        //
        // Onto the *driver's own* scale rather than a raw percentage — the
        // tyre band runs from their critical threshold (85 % by default) to a
        // fresh tyre, so a pad mapped onto 0..100 would land under the
        // threshold at 20 mm and draw a healthy set of pads in red with an
        // empty bar.
        let (wear, laps_rem) = if brakes_instead {
            // A GT3 pad starts near 29 mm and is a stop below 8. The
            // laps-remaining projection belongs to the tyre model and has no
            // counterpart here, so it stays "—" rather than being invented.
            let pad = phys.brake_pad_mm[i];
            let left = ((pad - 8.0) / 21.0).clamp(0.0, 1.0);
            (critical + left * (100.0 - critical), -1.0)
        } else {
            (phys.tyre_wear[i], app.engineer.stats.tyre_laps_remaining[i])
        };

        // "Calc..." was a sentence cut in half, and it appeared for a whole
        // stint — the projection needs a completed lap before it means
        // anything. Say which of the two it is.
        let laps_str = if laps_rem < 0.0 {
            // Not measured yet: the projection needs a completed lap.
            "—".to_string()
        } else if laps_rem <= 0.0 {
            "spent".tr(is_ru).to_string()
        } else if laps_rem >= 500.0 {
            "> 500".to_string()
        } else {
            tr_fmt("{0} laps", is_ru, &[&format!("{laps_rem:.0}")])
        };

        // Scaled between the driver's own critical threshold and a fresh tyre,
        // so an empty bar means "at the point you said is the end" rather than
        // "below 94 %", which is a number that used to be written in here and
        // showed a mid-stint tyre as dead.
        let span = (100.0 - critical).max(0.1);
        let health = ((wear - critical) / span).clamp(0.0, 1.0);

        // The same thresholds the engineer's advice uses, so the colour here
        // and the sentence there cannot disagree.
        let color = if wear >= warning {
            Color::Green
        } else if wear >= critical {
            Color::Yellow
        } else {
            Color::Red
        };

        // Three columns, not a label painted over the bar. `Gauge::label`
        // centres its text on top of the fill, so every row read as a sentence
        // half-swallowed by a coloured rectangle.
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(12),
                Constraint::Min(6),
                Constraint::Length(9),
            ])
            .split(*row);

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{name} "),
                    Style::default().fg(app.ui_state.get_color(&theme.text)),
                ),
                // Millimetres where the game measures a pad, per cent where it
                // measures a tyre: the same row, and never the wrong unit on
                // the wrong quantity.
                Span::styled(
                    if brakes_instead {
                        format!("{:.1}mm", phys.brake_pad_mm[i])
                    } else {
                        format!("{wear:.1}%")
                    },
                    Style::default().fg(color),
                ),
            ])),
            columns[0],
        );

        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
                .ratio(crate::ui::widgets::safe_ratio(health as f64))
                .label(""),
            columns[1],
        );

        f.render_widget(
            Paragraph::new(laps_str)
                .alignment(Alignment::Right)
                .style(Style::default().fg(Color::DarkGray)),
            columns[2],
        );
    }
}

fn render_environment(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    gfx: &ac_core::games::Session,
    phys: &ac_core::games::Car,
) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;

    let block = Block::default()
        .title(
            "Environment|as the terminal abbreviates it"
                .tr_lang(lang)
                .to_string(),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let fmt = app.config.formatter();

    // A game that does not publish grip leaves zero here, and "0.0%" in red is
    // not a missing number — it reads as an ice rink. Competizione is that
    // game: it reports how the track *is* by name instead, and turning that
    // into a percentage would be inventing the measurement this row exists to
    // show.
    let grip_measured = app
        .reading
        .as_ref()
        .is_none_or(|reading| reading.capabilities.track_grip);
    let grip_cell = if grip_measured {
        Cell::from(format!("{:.1}%", gfx.surface_grip * 100.0)).style(Style::default().fg(
            if gfx.surface_grip > 0.95 {
                Color::Green
            } else {
                Color::Red
            },
        ))
    } else {
        Cell::from("not measured".tr_lang(lang).to_string())
            .style(Style::default().fg(Color::DarkGray))
    };

    let rows = vec![
        Row::new(vec![
            Cell::from("Track Grip".tr_lang(lang).to_string()),
            grip_cell,
        ]),
        Row::new(vec![
            Cell::from("Air Temp".tr_lang(lang).to_string()),
            Cell::from(fmt.format_temp_prec(phys.air_temp_c, 1))
                .style(Style::default().fg(Color::Cyan)),
        ]),
        Row::new(vec![
            Cell::from("Road Temp".tr_lang(lang).to_string()),
            Cell::from(fmt.format_temp_prec(phys.road_temp_c, 1))
                .style(Style::default().fg(Color::Yellow)),
        ]),
        Row::new(vec![
            Cell::from("Wind Spd".tr_lang(lang).to_string()),
            Cell::from(format!("{:.1} km/h", gfx.wind_speed_kmh))
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
