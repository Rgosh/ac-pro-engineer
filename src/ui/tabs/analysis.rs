use ratatui::{prelude::*, widgets::*};
use crate::AppState;
use crate::ui::localization::tr;

pub fn render(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    // Если кругов нет - показываем заглушку
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

    // Основной макет: Список кругов (20%) | Детали (80%)
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
        
        // Макет правой части:
        // 1. Заголовок с WR/Delta (10%)
        // 2. График скорости (30%)
        // 3. Сектора и Статистика (30%)
        // 4. Отчет тренера (30%)
        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),      // Header
                Constraint::Percentage(35), // Chart
                Constraint::Percentage(30), // Sectors table
                Constraint::Min(10),        // Coach Report
            ])
            .split(main_layout[1]);
            
        render_header_stats(f, right_layout[0], app, selected_lap);
        render_speed_chart(f, right_layout[1], app, selected_lap, best_lap);
        
        // Разделяем среднюю часть на Сектора и Доп. Инфо
        let mid_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(right_layout[2]);
            
        render_sector_comparison(f, mid_layout[0], app, selected_lap, best_lap);
        render_extended_stats(f, mid_layout[1], app, selected_lap);
        
        render_coach_report(f, right_layout[3], app, selected_lap);
    }
}

// 1. Список кругов
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
    f.render_stateful_widget(list, area, &mut state);
}

// 2. Заголовок с рекордом
fn render_header_stats(f: &mut Frame, area: Rect, app: &AppState, lap: &crate::analyzer::LapData) {
    let theme = &app.ui_state.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    
    let inner = block.inner(area);
    f.render_widget(block, area);
    
    let wr_time = app.analyzer.world_record.as_ref().map(|r| r.time_ms).unwrap_or(0);
    let wr_text = if wr_time > 0 { format_ms(wr_time) } else { "--:--.---".into() };
    let wr_source = app.analyzer.world_record.as_ref().map(|r| r.source.clone()).unwrap_or("N/A".into());
    
    // Дельта относительно рекорда (если он есть) или 0
    let delta = if wr_time > 0 { (lap.lap_time_ms - wr_time) as f32 / 1000.0 } else { 0.0 };
    let delta_color = if delta <= 0.0 { Color::Green } else { Color::Red };
    
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left_text = Line::from(vec![
        Span::styled("Target Record: ", Style::default().fg(Color::Cyan)),
        Span::styled(format!("{} ({})", wr_text, wr_source), Style::default().add_modifier(Modifier::BOLD)),
    ]);
    
    let right_text = Line::from(vec![
        Span::styled("Your Time: ", Style::default().fg(Color::White)),
        Span::styled(format_ms(lap.lap_time_ms), Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  Delta: "),
        Span::styled(format!("{:.3}s", delta), Style::default().fg(delta_color).add_modifier(Modifier::BOLD)),
    ]);

    f.render_widget(Paragraph::new(left_text).alignment(Alignment::Left), layout[0]);
    f.render_widget(Paragraph::new(right_text).alignment(Alignment::Right), layout[1]);
}

// 3. График скорости
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
            .name(format!("Lap {}", selected.lap_number + 1))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Yellow))
            .graph_type(GraphType::Line)
            .data(&selected_data),
    ];
    
    if let Some(best_data) = &best_data_opt {
         datasets.push(Dataset::default()
            .name("Session Best")
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

// 4. Сравнение секторов (Таблица)
fn render_sector_comparison(f: &mut Frame, area: Rect, app: &AppState, selected: &crate::analyzer::LapData, best: Option<&crate::analyzer::LapData>) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == crate::config::Language::Russian;

    let block = Block::default()
        .title(if is_ru { "АНАЛИЗ СЕКТОРОВ И WR PACE" } else { "SECTOR ANALYSIS & WR PACE" })
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Идеальные сектора (Session Theoretical Best)
    let ideal_sectors = app.analyzer.best_sectors;
    let ideal_lap_sum: i32 = ideal_sectors.iter().filter(|&&x| x < i32::MAX && x > 0).sum();
    let ideal_valid = ideal_lap_sum > 0 && ideal_lap_sum < i32::MAX;

    let header = Row::new(vec![
        Cell::from(if is_ru { "Сектор" } else { "Sector" }),
        Cell::from(if is_ru { "Текущий" } else { "Current" }),
        Cell::from(if is_ru { "Лучший (Сессия)" } else { "Session Best" }),
        Cell::from(if is_ru { "Идеал (Target)" } else { "Ideal (Target)" }).style(Style::default().fg(Color::Magenta)),
        Cell::from(if is_ru { "Потеря" } else { "Loss" }),
    ]).style(Style::default().add_modifier(Modifier::BOLD).fg(app.ui_state.get_color(&theme.accent)));

    let mut rows = Vec::new();

    for i in 0..3 {
        let current_s = selected.sectors[i];
        let best_s = best.map(|l| l.sectors[i]).unwrap_or(0);
        let ideal_s = ideal_sectors[i];
        
        let loss = if ideal_valid && ideal_s < i32::MAX && current_s > 0 {
             (current_s - ideal_s) as f32 / 1000.0
        } else { 0.0 };
        
        // Цвет потери: Зеленый < 0.1s, Желтый < 0.5s, Красный > 0.5s
        let loss_color = if loss <= 0.05 { Color::Green } else if loss < 0.5 { Color::Yellow } else { Color::Red };

        rows.push(Row::new(vec![
            Cell::from(format!("S{}", i + 1)),
            Cell::from(if current_s > 0 { format_ms(current_s) } else { "-".into() }),
            Cell::from(if best_s > 0 { format_ms(best_s) } else { "-".into() }),
            Cell::from(if ideal_s < i32::MAX { format_ms(ideal_s) } else { "-".into() }).style(Style::default().fg(Color::Magenta)),
            Cell::from(if current_s > 0 { format!("+{:.3}s", loss) } else { "-".into() }).style(Style::default().fg(loss_color)),
        ]));
    }
    
    // Итоговая строка
    let total_loss = if ideal_valid && selected.lap_time_ms > 0 { (selected.lap_time_ms - ideal_lap_sum) as f32 / 1000.0 } else { 0.0 };
    
    rows.push(Row::new(vec![
        Cell::from("LAP").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from(format_ms(selected.lap_time_ms)).style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from(if let Some(b) = best { format_ms(b.lap_time_ms) } else { "-".into() }),
        Cell::from(if ideal_valid { format_ms(ideal_lap_sum) } else { "-".into() }).style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Cell::from(format!("+{:.3}s", total_loss)).style(Style::default().fg(if total_loss < 0.5 { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD)),
    ]));

    let table = Table::new(rows, [
        Constraint::Percentage(15),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(25),
    ]).header(header);

    f.render_widget(table, inner);
}

// 5. Расширенная статистика
fn render_extended_stats(f: &mut Frame, area: Rect, app: &AppState, selected: &crate::analyzer::LapData) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let block = Block::default()
        .title(tr("anal_stats_ext", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Формируем список параметров для отображения
    let params = vec![
        (tr("anal_max_spd", lang), format!("{:.0} km/h", selected.max_speed), Color::White),
        (tr("anal_avg_spd", lang), format!("{:.0} km/h", selected.avg_speed), Color::White),
        (tr("anal_full_thr", lang), format!("{:.1}%", selected.full_throttle_percent), Color::Green),
        (tr("anal_g_lat", lang), format!("{:.2}G", selected.peak_lat_g), Color::Magenta),
        (tr("anal_g_brake", lang), format!("{:.2}G", selected.peak_brake_g), Color::Red),
        (tr("anal_p_dev", lang), format!("{:.2} psi", selected.pressure_deviation), if selected.pressure_deviation > 1.0 { Color::Red } else { Color::Green }),
        (tr("anal_susp", lang), format!("{:.1}%", selected.suspension_travel_hist.iter().sum::<f32>()/4.0 * 1000.0), Color::Cyan), // Условная метрика
        ("Grip Usage".into(), format!("{:.1}%", selected.grip_usage_percent), if selected.grip_usage_percent > 80.0 { Color::Green } else { Color::Yellow }),
    ];

    let items: Vec<ListItem> = params.into_iter().map(|(label, val, color)| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("{}: ", label), Style::default().fg(Color::Gray)),
            Span::styled(val, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        ]))
    }).collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

// 6. Отчет Тренера (Coach Report)
fn render_coach_report(f: &mut Frame, area: Rect, app: &AppState, selected: &crate::analyzer::LapData) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == crate::config::Language::Russian;

    let block = Block::default()
        .title(tr("anal_comp_title", lang)) // "Comparison & Advice"
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Запускаем анализ
    let analysis = app.analyzer.analyze_standalone(selected);
    
    if analysis.is_perfect {
        let msg = Paragraph::new(tr("anal_self_perfect", lang))
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
    } else {
        // Рендерим список советов
        let items: Vec<ListItem> = analysis.advices.iter().map(|advice| {
            // Определяем цвет и иконку в зависимости от Severity
            let (color, icon) = match advice.severity {
                3 => (Color::Red, "✖"),    // Критично
                2 => (Color::Yellow, "⚠"), // Важно
                _ => (Color::Blue, "ℹ"),   // Инфо
            };
            
            // Форматируем: "[Иконка] Зона: Проблема" (1 строка) -> "   Fix: Решение" (2 строка)
            let content = vec![
                Line::from(vec![
                    Span::styled(format!("{} [{}]: ", icon, advice.zone), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                    Span::styled(&advice.problem, Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::raw("   ↳ "),
                    Span::styled(if is_ru { "Совет: " } else { "Fix: " }, Style::default().fg(Color::Gray)),
                    Span::styled(&advice.solution, Style::default().fg(Color::Green)),
                ]),
                Line::from(""), // Пустая строка для отступа
            ];
            ListItem::new(content)
        }).collect();
        
        let list = List::new(items)
            .block(Block::default()); // Без границ внутри, используем внешние
            
        f.render_widget(list, inner);
    }
}

// Вспомогательная функция форматирования времени
fn format_ms(ms: i32) -> String {
    let minutes = ms / 60000;
    let seconds = (ms % 60000) / 1000;
    let millis = ms % 1000;
    format!("{}:{:02}.{:03}", minutes, seconds, millis)
}