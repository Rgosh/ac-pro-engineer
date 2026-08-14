use crate::AppState;
use ac_core::engineer::Engineer;
use ac_core::i18n::Translate;
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState, engineer: &Engineer) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(14), Constraint::Min(0)])
        .split(area);

    let top_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(layout[0]);

    let clip_ratio = if engineer.stats.total_frames > 0 {
        engineer.stats.ffb_clip_frames as f32 / engineer.stats.total_frames as f32
    } else {
        0.0
    };

    let (last_steer, last_gas, last_brake, last_ffb) =
        if let Some(last) = engineer.stats.input_history.last() {
            (last.1, last.2, last.3, last.4)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

    let ffb_status_color;
    let recommendation;
    let status_text;

    if clip_ratio > 0.02 {
        ffb_status_color = Color::Red;
        status_text = "CRITICAL CLIPPING".tr(is_ru);
        recommendation = "Lower Game Gain immediately!".tr(is_ru);
    } else if clip_ratio > 0.001 {
        ffb_status_color = Color::Yellow;
        status_text = "LIGHT CLIPPING".tr(is_ru);
        recommendation = "Lower Gain slightly (2-3%)".tr(is_ru);
    } else if last_ffb.abs() < 0.6 && engineer.stats.total_frames > 300 {
        ffb_status_color = Color::Cyan;
        status_text = "WEAK SIGNAL".tr(is_ru);
        recommendation = "Safe to increase Gain".tr(is_ru);
    } else {
        ffb_status_color = Color::Green;
        status_text = "OPTIMAL".tr(is_ru);
        recommendation = "Settings are perfect".tr(is_ru);
    }

    let ffb_info_text = vec![
        Line::from(vec![
            Span::raw("Status: ".tr(is_ru)),
            Span::styled(
                status_text,
                Style::default()
                    .fg(ffb_status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Detail Loss (Clip): ".tr(is_ru)),
            Span::styled(
                format!("{:.1}%", clip_ratio * 100.0),
                Style::default().fg(if clip_ratio > 0.0 {
                    Color::Red
                } else {
                    Color::Green
                }),
            ),
        ]),
        Line::from(vec![
            Span::raw("Current Force: ".tr(is_ru)),
            Span::styled(
                format!("{:.0}%", last_ffb.abs() * 100.0),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "ADVICE:".tr(is_ru),
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(Span::styled(
            recommendation,
            Style::default().fg(ffb_status_color),
        )),
    ];

    let ffb_block = Paragraph::new(ffb_info_text)
        .block(
            Block::default()
                .title(" FFB Gain Tuning ".tr(is_ru))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ffb_status_color)),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    f.render_widget(ffb_block, top_row[0]);

    let steer_block = Block::default()
        .borders(Borders::ALL)
        .title(" Steering ".tr(is_ru));

    let canvas = Canvas::default()
        .block(steer_block)
        .x_bounds([-1.2, 1.2])
        .y_bounds([-1.2, 1.2])
        .paint(move |ctx| {
            ctx.draw(&ratatui::widgets::canvas::Circle {
                x: 0.0,
                y: 0.0,
                radius: 1.0,
                color: Color::DarkGray,
            });

            let angle = std::f64::consts::FRAC_PI_2 - (last_steer * std::f64::consts::PI * 1.5);

            let x = angle.cos() * 1.0;
            let y = angle.sin() * 1.0;

            ctx.draw(&CanvasLine {
                x1: 0.0,
                y1: 0.0,
                x2: x,
                y2: y,
                color: Color::Yellow,
            });

            ctx.draw(&ratatui::widgets::canvas::Circle {
                x: 0.0,
                y: 0.0,
                radius: 0.15,
                color: Color::Red,
            });
        });
    f.render_widget(canvas, top_row[1]);

    let bars_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(top_row[2].inner(&Margin {
            horizontal: 1,
            vertical: 1,
        }));

    f.render_widget(
        Block::default().title(" Input ").borders(Borders::ALL),
        top_row[2],
    );

    f.render_widget(
        Gauge::default()
            .ratio(crate::ui::widgets::safe_ratio(last_gas))
            .gauge_style(Style::default().fg(Color::Green))
            .label(format!("GAS {:.0}%", last_gas * 100.0)),
        bars_area[1],
    );

    f.render_widget(
        Gauge::default()
            .ratio(crate::ui::widgets::safe_ratio(last_brake))
            .gauge_style(Style::default().fg(Color::Red))
            .label(format!("BRK {:.0}%", last_brake * 100.0)),
        bars_area[2],
    );

    let ffb_bar_color = if last_ffb.abs() > 0.98 {
        Color::Red
    } else {
        Color::Gray
    };
    f.render_widget(
        Gauge::default()
            .ratio(crate::ui::widgets::safe_ratio(last_ffb.abs()))
            .gauge_style(Style::default().fg(ffb_bar_color))
            .label(format!("FFB {:.0}%", last_ffb.abs() * 100.0)),
        bars_area[3],
    );

    let graph_block = Block::default()
        .title(" Signal History (Last 10s) ".tr(is_ru))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let gas_data: Vec<(f64, f64)> = engineer
        .stats
        .input_history
        .iter()
        .map(|p| (p.0, p.2 * 100.0))
        .collect();
    let brake_data: Vec<(f64, f64)> = engineer
        .stats
        .input_history
        .iter()
        .map(|p| (p.0, p.3 * 100.0))
        .collect();
    let ffb_data: Vec<(f64, f64)> = engineer
        .stats
        .input_history
        .iter()
        .map(|p| (p.0, p.4.abs() * 100.0))
        .collect();

    let x_min = engineer
        .stats
        .input_history
        .first()
        .map(|p| p.0)
        .unwrap_or(0.0);
    let x_max = engineer
        .stats
        .input_history
        .last()
        .map(|p| p.0)
        .unwrap_or(100.0);

    let datasets = vec![
        Dataset::default()
            .name("Gas")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Green))
            .data(&gas_data),
        Dataset::default()
            .name("Brake")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Red))
            .data(&brake_data),
        Dataset::default()
            .name("FFB")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::DarkGray))
            .data(&ffb_data),
    ];

    let chart = Chart::new(datasets)
        .block(graph_block)
        .x_axis(Axis::default().bounds([x_min, x_max]).labels(vec![]))
        .y_axis(Axis::default().bounds([0.0, 100.0]).labels(vec![
            "0".into(),
            "50".into(),
            "100".into(),
        ]));

    f.render_widget(chart, layout[1]);
}
