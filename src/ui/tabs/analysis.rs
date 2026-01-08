use ratatui::{prelude::*, widgets::*};
use crate::AppState;
use crate::ui::localization::tr;

pub fn render(f: &mut Frame, area: Rect, app: &AppState) {
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
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(80),
        ])
        .split(area);
        
    render_laps_list(f, main_layout[0], app);
    
    let selected_idx = app.ui_state.setup_list_state.selected().unwrap_or(0);
    
    if selected_idx < app.analyzer.laps.len() {
        let selected_lap = &app.analyzer.laps[selected_idx];
        let best_lap = app.analyzer.best_lap_index.map(|i| &app.analyzer.laps[i]);
        
        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30), // График Скорости
                Constraint::Percentage(20), // График Дельты / Ввода
                Constraint::Percentage(25), // Анализ / Советы
                Constraint::Min(0),         // Статистика
            ])
            .split(main_layout[1]);
            
        render_speed_chart(f, right_layout[0], app, selected_lap, best_lap);
        render_delta_inputs_chart(f, right_layout[1], app, selected_lap, best_lap);
        render_analysis_panel(f, right_layout[2], app, selected_lap, best_lap);
        render_extended_stats(f, right_layout[3], app, selected_lap);
    }
}

fn render_laps_list(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let block = Block::default()
        .title(tr("anal_laps_list", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
    let items: Vec<ListItem> = app.analyzer.laps.iter().enumerate().map(|(i, lap)| {
        let is_best = Some(i) == app.analyzer.best_lap_index;
        let time_str = format_ms(lap.lap_time_ms);
        let prefix = if is_best { "★" } else { " " };
        
        let mut style = Style::default().fg(app.ui_state.get_color(&theme.text));
        if is_best {
            style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
        } else if !lap.valid {
            style = style.fg(Color::Red);
        }
        
        ListItem::new(format!("{} L{}: {}", prefix, lap.lap_number + 1, time_str))
            .style(style)
    }).collect();
    
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(app.ui_state.get_color(&theme.highlight)).fg(Color::Black));
    
    let mut state = app.ui_state.setup_list_state.clone();
    if state.selected().is_none() && !app.analyzer.laps.is_empty() {
        state.select(Some(app.analyzer.laps.len() - 1));
    }
    
    f.render_stateful_widget(list, area, &mut state);
}

fn render_speed_chart(f: &mut Frame, area: Rect, app: &AppState, selected: &crate::analyzer::LapData, best: Option<&crate::analyzer::LapData>) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    
    let selected_data: Vec<(f64, f64)> = selected.telemetry_trace.iter()
        .map(|p| (p.distance as f64, p.speed as f64))
        .collect();
    
    let best_data_opt: Option<Vec<(f64, f64)>> = if let Some(best_l) = best {
        if selected.lap_number != best_l.lap_number {
            Some(best_l.telemetry_trace.iter()
                .map(|p| (p.distance as f64, p.speed as f64))
                .collect())
        } else { None }
    } else { None };

    let mut datasets = vec![
        Dataset::default()
            .name(format!("L{}", selected.lap_number + 1))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Yellow))
            .graph_type(GraphType::Line)
            .data(&selected_data),
    ];
    
    if let Some(best_data) = &best_data_opt {
         datasets.push(Dataset::default()
            .name("Best")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::DarkGray)) 
            .graph_type(GraphType::Line)
            .data(best_data));
    }
    
    let chart = Chart::new(datasets)
        .block(Block::default()
            .title(tr("anal_speed_comp", lang))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))))
        .x_axis(Axis::default().bounds([0.0, 1.0]).labels(vec![]))
        .y_axis(Axis::default().bounds([0.0, 350.0]).labels(vec![Span::raw("0"), Span::raw("350")]));
            
    f.render_widget(chart, area);
}

fn render_delta_inputs_chart(f: &mut Frame, area: Rect, app: &AppState, selected: &crate::analyzer::LapData, _best: Option<&crate::analyzer::LapData>) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    
    let inputs_data: Vec<(f64, f64)> = selected.telemetry_trace.iter()
        .map(|p| {
            let val = if p.brake > 0.0 { -p.brake as f64 } else { p.gas as f64 };
            (p.distance as f64, val)
        })
        .collect();

    let chart = Chart::new(vec![
        Dataset::default()
            .name(tr("anal_inputs", lang))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&inputs_data)
    ])
    .block(Block::default()
        .title(tr("anal_inputs", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))))
    .x_axis(Axis::default().bounds([0.0, 1.0]))
    .y_axis(Axis::default().bounds([-1.0, 1.0]).labels(vec![Span::raw("Brk"), Span::raw("Gas")]));
    
    f.render_widget(chart, area);
}

fn render_analysis_panel(f: &mut Frame, area: Rect, app: &AppState, selected: &crate::analyzer::LapData, best: Option<&crate::analyzer::LapData>) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    
    let block = Block::default()
        .title(tr("anal_comp_title", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
    let inner = block.inner(area);
    f.render_widget(block, area);

    let is_best_selected = if let Some(best_l) = best { selected.lap_number == best_l.lap_number } else { true };

    if is_best_selected {
        // АНАЛИЗ ЛУЧШЕГО КРУГА (Standalone)
        let analysis = app.analyzer.analyze_standalone(selected);
        let mut lines = vec![];
        
        lines.push(Line::from(vec![
            Span::styled(tr("anal_self_title", lang), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ]));
        
        if analysis.is_perfect {
            lines.push(Line::from(tr("anal_self_perfect", lang)).style(Style::default().fg(Color::Green)));
        } else {
             for adv in analysis.advices {
                 lines.push(Line::from(vec![
                     Span::raw("• "),
                     Span::styled(tr(&adv, lang), Style::default().fg(Color::Yellow)),
                 ]));
             }
        }
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);

    } else if let Some(best_l) = best {
        // СРАВНЕНИЕ С ЛУЧШИМ
        let comparison = app.analyzer.compare_laps(selected, best_l);
        let layout = Layout::default().direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(33)])
            .split(inner);
            
        let stats_loss = vec![
            Line::from(vec![Span::raw(format!("{}: ", tr("anal_reason_brake", lang))), Span::styled(format!("-{:.2}s", comparison.lost_on_braking), Style::default().fg(Color::Red))]),
            Line::from(vec![Span::raw(format!("{}: ", tr("anal_reason_corner", lang))), Span::styled(format!("-{:.2}s", comparison.lost_in_corners), Style::default().fg(Color::Red))]),
            Line::from(vec![Span::raw(format!("{}: ", tr("anal_reason_straight", lang))), Span::styled(format!("-{:.2}s", comparison.lost_on_straights), Style::default().fg(Color::Red))]),
        ];
        
        let mut advice = Vec::new();
        if comparison.lost_on_braking > 0.2 { advice.push(Line::from(format!("• {}", tr("anal_advice_brake", lang))).style(Style::default().fg(Color::Yellow))); }
        if comparison.lost_in_corners > 0.3 { advice.push(Line::from(format!("• {}", tr("anal_advice_corner", lang))).style(Style::default().fg(Color::Yellow))); }
        if comparison.lost_on_straights > 0.2 { advice.push(Line::from(format!("• {}", tr("anal_advice_gas", lang))).style(Style::default().fg(Color::Yellow))); }
        
        let delta_color = if comparison.time_diff > 0.0 { Color::Red } else { Color::Green };
        let summary = vec![
            Line::from(vec![Span::raw("Delta: "), Span::styled(format!("{:.3}s", comparison.time_diff), Style::default().fg(delta_color).add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::raw("Spd Diff: "), Span::styled(format!("{:.1} km/h", comparison.speed_diff_avg), Style::default().fg(Color::Cyan))]),
        ];

        f.render_widget(Paragraph::new(stats_loss).block(Block::default().borders(Borders::RIGHT)), layout[0]);
        f.render_widget(Paragraph::new(advice).block(Block::default().borders(Borders::RIGHT)), layout[1]);
        f.render_widget(Paragraph::new(summary), layout[2]);
    }
}

fn render_extended_stats(f: &mut Frame, area: Rect, app: &AppState, lap: &crate::analyzer::LapData) {
    let lang = &app.config.language;
    let theme = &app.ui_state.theme;
    
    let block = Block::default()
        .title(tr("anal_stats_ext", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
    let inner = block.inner(area);
    f.render_widget(block, area);
    
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(25), Constraint::Percentage(25), Constraint::Percentage(25)])
        .split(inner);
        
    let col1 = vec![
        Line::from(vec![Span::raw(format!("{}: ", tr("anal_max_spd", lang))), Span::styled(format!("{:.0}", lap.max_speed), Style::default().fg(Color::White))]),
        Line::from(vec![Span::raw(format!("{}: ", tr("anal_avg_spd", lang))), Span::styled(format!("{:.0}", lap.avg_speed), Style::default().fg(Color::White))]),
    ];
    let col2 = vec![
        Line::from(vec![Span::raw(format!("{}: ", tr("anal_full_thr", lang))), Span::styled(format!("{:.0}%", lap.full_throttle_percent), Style::default().fg(Color::Green))]),
        Line::from(vec![Span::raw(format!("{}: ", tr("lbl_coast", lang))), Span::styled(format!("{:.0}%", lap.coasting_percent), Style::default().fg(if lap.coasting_percent > 5.0 { Color::Red } else { Color::Green }))]),
    ];
    let col3 = vec![
        Line::from(vec![Span::raw(format!("{}: ", tr("anal_g_lat", lang))), Span::styled(format!("{:.2}G", lap.peak_lat_g), Style::default().fg(Color::Magenta))]),
        Line::from(vec![Span::raw(format!("{}: ", tr("anal_g_brake", lang))), Span::styled(format!("{:.2}G", lap.peak_brake_g), Style::default().fg(Color::Red))]),
    ];
    let col4 = vec![
        Line::from(vec![Span::raw(format!("{}: ", tr("lbl_smooth", lang))), crate::ui::widgets::render_progress_bar(lap.throttle_smoothness, 100.0)]),
        Line::from(vec![Span::raw(format!("{}: ", tr("lbl_trail", lang))), crate::ui::widgets::render_progress_bar(lap.trail_braking_score, 100.0)]),
    ];

    f.render_widget(Paragraph::new(col1), layout[0]);
    f.render_widget(Paragraph::new(col2), layout[1]);
    f.render_widget(Paragraph::new(col3), layout[2]);
    f.render_widget(Paragraph::new(col4), layout[3]);
}

fn format_ms(ms: i32) -> String {
    let minutes = ms / 60000;
    let seconds = (ms % 60000) / 1000;
    let millis = ms % 1000;
    format!("{}:{:02}.{:03}", minutes, seconds, millis)
}