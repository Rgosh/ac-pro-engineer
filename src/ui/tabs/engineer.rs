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
    let text = Paragraph::new("Vertical Engineer View")
        .block(
            Block::default()
                .title(tr("tab_eng", &app.config.language))
                .borders(Borders::ALL),
        )
        .alignment(Alignment::Center);
    f.render_widget(text, area);
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
                format!("{}", app.engineer.stats.lockup_frames),
                Style::default().fg(if app.engineer.stats.lockup_frames > 10 {
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
