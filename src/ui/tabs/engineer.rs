use crate::ui::localization::tr;
use crate::ui::widgets::*;
use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render_horizontal(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    render_recommendations(f, layout[0], app);
    render_analysis(f, layout[1], app);
}

pub fn render_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let is_ru = app.config.language == crate::config::Language::Russian;

    let block = Block::default()
        .title(if is_ru {
            "Мастер Настройки"
        } else {
            "Setup Wizard"
        })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);

    let phase_str = match app.engineer.wizard_phase {
        crate::engineer::WizardPhase::Entry => {
            if is_ru {
                "ВХОД (Entry)"
            } else {
                "ENTRY"
            }
        }
        crate::engineer::WizardPhase::Apex => {
            if is_ru {
                "АПЕКС (Apex)"
            } else {
                "APEX"
            }
        }
        crate::engineer::WizardPhase::Exit => {
            if is_ru {
                "ВЫХОД (Exit)"
            } else {
                "EXIT"
            }
        }
    };

    let problem_str = match app.engineer.wizard_problem {
        crate::engineer::WizardProblem::Understeer => {
            if is_ru {
                "СНОС (Under)"
            } else {
                "UNDERSTEER"
            }
        }
        crate::engineer::WizardProblem::Oversteer => {
            if is_ru {
                "ЗАНОС (Over)"
            } else {
                "OVERSTEER"
            }
        }
        crate::engineer::WizardProblem::Instability => {
            if is_ru {
                "НЕСТАБИЛЬНОСТЬ"
            } else {
                "INSTABILITY"
            }
        }
    };

    let controls_text = format!(
        " PHASE: < {} >  |  PROBLEM: < {} >  (Use Arrows to change)",
        phase_str, problem_str
    );

    f.render_widget(
        Paragraph::new(controls_text)
            .style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::BOTTOM)),
        layout[0],
    );

    let advice = app.engineer.get_wizard_advice();

    let items: Vec<ListItem<'_>> = advice
        .iter()
        .map(|s| {
            ListItem::new(Line::from(vec![
                Span::styled(" • ", Style::default().fg(Color::Yellow)),
                Span::raw(s),
            ]))
        })
        .collect();

    f.render_widget(
        List::new(items).block(Block::default().padding(Padding::new(2, 2, 1, 1))),
        layout[1],
    );
}

fn render_recommendations(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("eng_recs", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    if app.recommendations.is_empty() {
        let message = Paragraph::new(vec![
            Line::from(""),
            Line::from(tr("eng_good", lang)),
            Line::from(""),
            Line::from(tr("eng_push", lang)),
        ])
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center);

        f.render_widget(message, area);
    } else {
        let items: Vec<ListItem<'_>> = app
            .recommendations
            .iter()
            .take(8)
            .map(|rec| {
                let severity_color = match rec.severity {
                    crate::engineer::Severity::Critical => Color::Red,
                    crate::engineer::Severity::Warning => Color::Yellow,
                    crate::engineer::Severity::Info => Color::Blue,
                };

                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("● ", Style::default().fg(severity_color)),
                        Span::styled(
                            &rec.component,
                            Style::default().fg(app.ui_state.get_color(&theme.highlight)),
                        ),
                        Span::raw(" - "),
                        Span::styled(
                            &rec.message,
                            Style::default().fg(app.ui_state.get_color(&theme.text)),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(
                            format!("{}: ", tr("eng_action", lang)),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            &rec.action,
                            Style::default().fg(app.ui_state.get_color(&theme.accent)),
                        ),
                    ]),
                ];

                for param in &rec.parameters {
                    lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::styled(
                            format!("{}: ", param.name),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("{:.1}{}", param.current, param.unit),
                            Style::default().fg(Color::White),
                        ),
                        Span::raw(" → "),
                        Span::styled(
                            format!("{:.1}{}", param.target, param.unit),
                            Style::default().fg(Color::Green),
                        ),
                    ]));
                }

                lines.push(Line::from(""));
                ListItem::new(lines)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .style(Style::default().fg(app.ui_state.get_color(&theme.text)));

        f.render_widget(list, area);
    }
}

fn render_analysis(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title(tr("eng_analysis", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);

    let total_lockups =
        app.engineer.stats.lockup_frames_front + app.engineer.stats.lockup_frames_rear;

    let analysis = vec![
        Line::from(vec![
            Span::styled(
                format!("{}: ", tr("eng_smooth", lang)),
                Style::default().fg(app.ui_state.get_color(&theme.text)),
            ),
            render_progress_bar(app.engineer.driving_style.smoothness, 100.0),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", tr("eng_aggr", lang)),
                Style::default().fg(app.ui_state.get_color(&theme.text)),
            ),
            render_progress_bar(app.engineer.driving_style.aggression * 100.0, 100.0),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", tr("eng_trail", lang)),
                Style::default().fg(app.ui_state.get_color(&theme.text)),
            ),
            render_progress_bar(app.engineer.driving_style.trail_braking * 100.0, 100.0),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{}: ", tr("eng_lock", lang)),
                Style::default().fg(app.ui_state.get_color(&theme.text)),
            ),
            Span::styled(
                format!("{}", total_lockups),
                Style::default().fg(if total_lockups > 10 {
                    Color::Red
                } else {
                    Color::Green
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", tr("eng_spin", lang)),
                Style::default().fg(app.ui_state.get_color(&theme.text)),
            ),
            Span::styled(
                format!("{}", app.engineer.stats.wheel_spin_frames),
                Style::default().fg(if app.engineer.stats.wheel_spin_frames > 15 {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            ),
        ]),
    ];

    let analysis_widget = Paragraph::new(analysis).block(Block::default());

    f.render_widget(analysis_widget, inner);
    f.render_widget(block, area);
}
