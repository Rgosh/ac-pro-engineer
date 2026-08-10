//! Where the lap went, corner by corner.
//!
//! The rest of the Analysis tab draws what happened. This one says **where** —
//! which corner cost the time, and then, for the worst of them, what the driver
//! did differently there. A driver who knows they lost four tenths somewhere
//! has learned nothing; a driver who knows three of them went in T3, on the
//! brakes, has somewhere to go.
//!
//! Two rules run through all of it, both from `docs/plan-0.3.7-analysis.md`:
//!
//! * **A corner with no match in the reference is not a delta of zero.** It is
//!   drawn as `—`, because the alternative is inventing a comparison.
//! * **The filter is the feature.** Twenty corners with a number beside each is
//!   another table to read. Three corners that cost a tenth each is a job.

use crate::AppState;
use ac_core::analyzer::LapData;
use ac_core::corners::{CornerComparison, Decomposition};
use ratatui::{prelude::*, widgets::*};

/// A corner has to cost more than this to be worth naming, in seconds.
///
/// A hundredth is noise — a slightly different line, a sample landing either
/// side of a boundary. A tenth is a corner to go and work on.
pub const LOSS_THRESHOLD_S: f32 = 0.10;

pub fn render(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap: &LapData,
    reference: Option<&LapData>,
) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;
    let border = Style::default().fg(app.ui_state.get_color(&theme.border));

    let Some(reference) = reference else {
        message(
            f,
            area,
            border,
            if is_ru {
                "Нужен эталонный круг. Проедьте второй круг или загрузите сохранённый ('L')."
            } else {
                "Needs a reference lap. Drive a second one, or load a saved lap with 'L'."
            },
        );
        return;
    };

    // The same lap compared with itself is every delta at zero, which reads as
    // a perfect lap rather than as the tautology it is.
    if std::ptr::eq(lap, reference) {
        message(
            f,
            area,
            border,
            if is_ru {
                "Это и есть эталонный круг — сравнивать не с чем."
            } else {
                "This is the reference lap — there is nothing to compare it with."
            },
        );
        return;
    }

    let decomposition = app
        .ui_state
        .analysis
        .corner_cache
        .borrow_mut()
        .get_or_compute(lap, reference);

    if decomposition.sections.is_empty() {
        message(
            f,
            area,
            border,
            if is_ru {
                "В трейсе не найдено ни одного поворота. Круг слишком короткий или без телеметрии."
            } else {
                "No corners found in the trace — too short a lap, or no telemetry in it."
            },
        );
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(9),
        ])
        .split(area);

    render_header(f, layout[0], app, lap, reference, &decomposition, is_ru);
    render_table(f, layout[1], app, &decomposition, is_ru);
    render_detail(f, layout[2], app, lap, &decomposition, is_ru);
}

fn message(f: &mut Frame<'_>, area: Rect, border: Style, text: &str) {
    let block = Block::default().borders(Borders::ALL).border_style(border);
    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(block),
        area,
    );
}

/// `+0.412` — with the sign, always, because a delta without one is a duration.
fn signed(delta_ms: i32) -> String {
    format!(
        "{}{:.3}",
        if delta_ms < 0 { "-" } else { "+" },
        (delta_ms.abs() as f32) / 1000.0
    )
}

fn lap_time(ms: i32) -> String {
    format!("{}:{:02}.{:03}", ms / 60000, (ms % 60000) / 1000, ms % 1000)
}

fn delta_colour(delta_ms: i32) -> Color {
    let threshold = (LOSS_THRESHOLD_S * 1000.0) as i32;
    if delta_ms > threshold {
        Color::Red
    } else if delta_ms > 0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn render_header(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap: &LapData,
    reference: &LapData,
    decomposition: &Decomposition,
    is_ru: bool,
) {
    let theme = &app.ui_state.theme;
    let filtered = app.ui_state.analysis.corners_filter;

    let corners = decomposition.sections.len();
    let losses = decomposition.losses_over(LOSS_THRESHOLD_S).len();

    let title = if is_ru {
        format!(
            " Круг {}  {}   против {}   {} поворотов, {} стоящих внимания ",
            lap.lap_number + 1,
            lap_time(lap.lap_time_ms),
            lap_time(reference.lap_time_ms),
            corners,
            losses
        )
    } else {
        format!(
            " Lap {}  {}   vs {}   {} corners, {} worth looking at ",
            lap.lap_number + 1,
            lap_time(lap.lap_time_ms),
            lap_time(reference.lap_time_ms),
            corners,
            losses
        )
    };

    let mut spans = vec![
        Span::styled(
            signed(decomposition.total_ms),
            Style::default()
                .fg(delta_colour(decomposition.total_ms))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if is_ru {
            "  на круге"
        } else {
            "  on the lap"
        }),
    ];

    // The run to the first corner is time too, and hiding it would leave the
    // corner deltas not adding up to the lap.
    if decomposition.opening_ms.abs() >= 10 {
        spans.push(Span::styled(
            format!(
                "   {} {}",
                if is_ru { "до Т1" } else { "to T1" },
                signed(decomposition.opening_ms)
            ),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if filtered {
        spans.push(Span::styled(
            if is_ru {
                format!("   [только потери > {LOSS_THRESHOLD_S:.2}s]")
            } else {
                format!("   [losses over {LOSS_THRESHOLD_S:.2}s only]")
            },
            Style::default().fg(Color::Cyan),
        ));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
        ),
        area,
    );
}

fn render_table(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    decomposition: &Decomposition,
    is_ru: bool,
) {
    let theme = &app.ui_state.theme;
    let filtered = app.ui_state.analysis.corners_filter;

    let shown: Vec<&CornerComparison> = if filtered {
        decomposition.losses_over(LOSS_THRESHOLD_S)
    } else {
        decomposition.sections.iter().collect()
    };

    if shown.is_empty() {
        message(
            f,
            area,
            Style::default().fg(app.ui_state.get_color(&theme.border)),
            if is_ru {
                "Ни один поворот не стоил больше десятой. Хороший круг."
            } else {
                "No corner cost more than a tenth. That was a tidy lap."
            },
        );
        return;
    }

    let rows: Vec<Row<'_>> = shown
        .iter()
        .map(|section| {
            let corner = &section.corner;
            let matched = section.reference.is_some();

            // No corner at this distance in the reference is no comparison. The
            // section still has a time delta — it is a stretch of track either
            // way — but nothing about the corner itself can be said.
            let speed = match section.speed_deltas() {
                Some((entry, min, exit)) => {
                    format!("{entry:+.0} / {min:+.0} / {exit:+.0}")
                }
                None => "—".to_string(),
            };

            Row::new(vec![
                Cell::from(corner.label()).style(
                    Style::default()
                        .fg(app.ui_state.get_color(&theme.text))
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(corner.direction.arrow().to_string()),
                Cell::from(format!("{:.0}", corner.min_speed)),
                Cell::from(speed).style(Style::default().fg(if matched {
                    app.ui_state.get_color(&theme.text)
                } else {
                    Color::DarkGray
                })),
                Cell::from(signed(section.delta_ms)).style(
                    Style::default()
                        .fg(delta_colour(section.delta_ms))
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        })
        .collect();

    let header = if is_ru {
        ["Пов", "", "Мин", "Скор: вход/мин/выход", "Δ"]
    } else {
        ["Cnr", "", "Min", "Speed: in/min/out", "Δ"]
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(22),
            Constraint::Length(9),
        ],
    )
    .header(
        Row::new(header).style(
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(if is_ru {
                " Где ушло время "
            } else {
                " Where the time went "
            })
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))),
    );

    f.render_widget(table, area);
}

/// The worst corner, spelled out.
///
/// One corner rather than all of them: this is the paragraph a driver reads
/// before going back out, and five of them side by side is the wall of numbers
/// the whole sub-tab exists to replace.
fn render_detail(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap: &LapData,
    decomposition: &Decomposition,
    is_ru: bool,
) {
    let theme = &app.ui_state.theme;
    let border = Style::default().fg(app.ui_state.get_color(&theme.border));

    let worst = decomposition.losses_over(LOSS_THRESHOLD_S);
    let Some(section) = worst.first() else {
        message(
            f,
            area,
            border,
            if is_ru {
                "Нечего разбирать — ни один поворот не стоил больше десятой."
            } else {
                "Nothing to pull apart — no corner cost more than a tenth."
            },
        );
        return;
    };

    let corner = &section.corner;
    let mut lines: Vec<Line<'_>> = Vec::new();

    let heading = if is_ru {
        format!(
            "{} {} — потеряно {:.2} с",
            corner.label(),
            corner.direction.arrow(),
            section.delta_ms as f32 / 1000.0
        )
    } else {
        format!(
            "{} {} — {:.2} s lost",
            corner.label(),
            corner.direction.arrow(),
            section.delta_ms as f32 / 1000.0
        )
    };
    lines.push(Line::from(Span::styled(
        heading,
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )));

    if section.reference.is_none() {
        lines.push(Line::from(Span::styled(
            if is_ru {
                "  Эталонный круг не проходил здесь поворот — сравнить не с чем."
            } else {
                "  The reference lap has no corner here, so there is nothing to compare."
            },
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let mut row = |label: &str, value: String, good: bool| {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {label:<16}"),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    value,
                    Style::default().fg(if good { Color::Green } else { Color::Yellow }),
                ),
            ]));
        };

        // Metres need a track to be metres of. A lap saved before the track
        // length was recorded has none, and saying "0 m" would be a lie about
        // the braking point rather than an absent number.
        match section.braking_delta_m(lap.track_length_m) {
            // Under a metre apart is the same braking point. "0 m earlier"
            // reads as a measurement rather than as the two being identical.
            Some(metres) if metres.abs() < 1.0 => row(
                if is_ru {
                    "Торможение"
                } else {
                    "Braking"
                },
                if is_ru {
                    "в той же точке".to_string()
                } else {
                    "at the same point".to_string()
                },
                true,
            ),
            Some(metres) => row(
                if is_ru {
                    "Торможение"
                } else {
                    "Braking"
                },
                if is_ru {
                    format!(
                        "на {:.0} м {}",
                        metres.abs(),
                        if metres > 0.0 {
                            "позже"
                        } else {
                            "раньше"
                        }
                    )
                } else {
                    format!(
                        "{:.0} m {}",
                        metres.abs(),
                        if metres > 0.0 { "later" } else { "earlier" }
                    )
                },
                metres.abs() < 5.0,
            ),
            None => row(
                if is_ru {
                    "Торможение"
                } else {
                    "Braking"
                },
                if is_ru {
                    "нет данных".to_string()
                } else {
                    "not measured".to_string()
                },
                true,
            ),
        }

        if let Some((entry, min, exit)) = section.speed_deltas() {
            row(
                if is_ru { "Вход" } else { "Entry speed" },
                format!("{entry:+.1} km/h"),
                entry.abs() < 2.0,
            );
            row(
                if is_ru {
                    "Минимальная"
                } else {
                    "Minimum speed"
                },
                format!("{min:+.1} km/h"),
                min >= -1.0,
            );
            row(
                if is_ru { "Выход" } else { "Exit speed" },
                format!("{exit:+.1} km/h"),
                exit >= -1.0,
            );
        }

        // Throttle is the one measured in time rather than distance: how long
        // after the slowest point the driver picked the power back up.
        match section.throttle_delta_s() {
            Some(delta) => row(
                if is_ru { "Газ" } else { "Throttle" },
                if is_ru {
                    format!(
                        "на {:.2} с {}",
                        delta.abs(),
                        if delta > 0.0 {
                            "позже"
                        } else {
                            "раньше"
                        }
                    )
                } else {
                    format!(
                        "{:.2} s {}",
                        delta.abs(),
                        if delta > 0.0 { "later" } else { "earlier" }
                    )
                },
                delta <= 0.05,
            ),
            None if corner.throttle_delay_s().is_none() => row(
                if is_ru { "Газ" } else { "Throttle" },
                if is_ru {
                    "не вернулся к газу в повороте".to_string()
                } else {
                    "never got back to throttle in the corner".to_string()
                },
                false,
            ),
            None => {}
        }
    }

    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(if is_ru {
                    " Худший поворот "
                } else {
                    " The worst corner "
                })
                .borders(Borders::ALL)
                .border_style(border),
        ),
        area,
    );
}
