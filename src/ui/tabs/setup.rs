use ratatui::{prelude::*, widgets::*};
use crate::AppState;
use crate::setup_manager::CarSetup;
use crate::ui::localization::tr;

pub fn render(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let setups = app.setup_manager.get_setups();
    let best_setup_idx = app.setup_manager.get_best_match_index();
    
    // Главный контейнер
    let main_block = Block::default()
        .title(tr("tab_setup", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    
    let inner = main_block.inner(area);
    f.render_widget(main_block, area);
    
    // Макет: 30% Список, 70% Детали
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .split(inner);
        
    // --- Левая панель: Список сетапов ---
    render_setup_list(f, layout[0], app, &setups, best_setup_idx);
    
    // --- Правая панель: Сравнение и Инженер ---
    if let Some(selected_idx) = app.ui_state.setup_list_state.selected() {
        if selected_idx < setups.len() {
            let selected_setup = &setups[selected_idx];
            
            // Если есть лучший сетап и он не выбран, мы сравниваем с ним.
            // Иначе (если выбран лучший или лучшего нет) сравниваем с Live (или просто не показываем diff).
            let reference_setup = if let Some(best_idx) = best_setup_idx {
                if best_idx != selected_idx {
                    setups.get(best_idx)
                } else {
                    None // Выбран лучший - сравнение не нужно
                }
            } else {
                None
            };
            
            let right_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6), // Блок инженера стал больше
                    Constraint::Min(0),    // Таблица
                ])
                .split(layout[1]);
                
            render_engineer_advice(
                f, 
                right_layout[0], 
                app, 
                selected_setup, 
                reference_setup, 
                selected_idx
            );
            
            render_comparison_table(
                f, 
                right_layout[1], 
                app, 
                selected_setup, 
                reference_setup
            );
        }
    } else if !setups.is_empty() {
        // Хак: Если ничего не выбрано, но список не пуст, рендерим первый элемент (при старте)
        let selected_setup = &setups[0];
        let reference_setup = if let Some(best_idx) = best_setup_idx {
            if best_idx != 0 { setups.get(best_idx) } else { None }
        } else { None };
        
        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Min(0),
            ])
            .split(layout[1]);
            
        render_engineer_advice(f, right_layout[0], app, selected_setup, reference_setup, 0);
        render_comparison_table(f, right_layout[1], app, selected_setup, reference_setup);
    } else {
        // Если файлов нет
        let no_data = Paragraph::new(tr("set_no_file", lang))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::LEFT));
        f.render_widget(no_data, layout[1]);
    }
}

fn render_setup_list(f: &mut Frame, area: Rect, app: &AppState, setups: &[CarSetup], best_idx: Option<usize>) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let block = Block::default()
        .title(tr("set_list", lang))
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
    let items: Vec<ListItem> = setups.iter().enumerate().map(|(i, setup)| {
        let is_best = Some(i) == best_idx;
        
        // Звездочка для лучшего сетапа
        let (icon, color, name_style) = if is_best {
            ("★", Color::Green, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        } else {
            ("•", app.ui_state.get_color(&theme.text), Style::default().fg(app.ui_state.get_color(&theme.text)))
        };
        
        let content = Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(color)),
            Span::styled(format!("{}", setup.name), name_style),
            Span::styled(format!(" ({})", setup.source), Style::default().fg(Color::DarkGray)),
        ]);
        
        ListItem::new(content)
    }).collect();
    
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default()
            .bg(app.ui_state.get_color(&theme.highlight))
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD));
            
    // Мы клонируем стейт только для рендера, реальное управление в main.rs
    let mut state = app.ui_state.setup_list_state.clone(); 
    f.render_stateful_widget(list, area, &mut state);
}

fn render_engineer_advice(f: &mut Frame, area: Rect, app: &AppState, selected: &CarSetup, best: Option<&CarSetup>, _selected_idx: usize) {
    let lang = &app.config.language;
    let is_ru = *lang == crate::config::Language::Russian;
    
    let (block_color, title, lines) = if let Some(best_setup) = best {
        // Мы смотрим НЕ на лучший сетап. Сравниваем Selected vs Best.
        let mut advice_lines = vec![
            Line::from(vec![
                Span::styled(if is_ru { "⚠ СОВЕТ: " } else { "⚠ ADVICE: " }, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(if is_ru { 
                    format!("Рекомендуется использовать '{}'. Отличия:", best_setup.name) 
                } else { 
                    format!("Recommended: '{}'. Differences:", best_setup.name) 
                }),
            ]),
        ];
        
        // Получаем конкретные советы от инженера
        let diffs = app.engineer.compare_setups_advice(selected, best_setup);
        for diff in diffs {
             advice_lines.push(Line::from(vec![
                Span::raw(" • "),
                Span::styled(diff, Style::default().fg(Color::White)),
            ]));
        }
        
        (Color::Yellow, if is_ru { "АНАЛИЗ СЕТАПА" } else { "SETUP ANALYSIS" }, advice_lines)
    } else {
        // Мы смотрим на ЛУЧШИЙ сетап.
        (Color::Green, if is_ru { "ВЕРДИКТ ИНЖЕНЕРА" } else { "ENGINEER VERDICT" }, vec![
            Line::from(vec![
                Span::styled(if is_ru { "✓ ОТЛИЧНЫЙ ВЫБОР" } else { "✓ EXCELLENT CHOICE" }, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(if is_ru { 
                "Этот сетап наиболее подходит для текущей трассы." 
            } else { 
                "This setup is the best match for the current track." 
            }),
            Line::from(""),
            Line::from(if is_ru { 
                "Инженер: Проверьте давление шин после 2-3 кругов." 
            } else { 
                "Engineer: Check tyre pressures after 2-3 laps warmup." 
            }),
        ])
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(block_color));
        
    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true });
        
    f.render_widget(p, area);
}

fn render_comparison_table(f: &mut Frame, area: Rect, app: &AppState, selected: &CarSetup, reference: Option<&CarSetup>) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == crate::config::Language::Russian;
    
    let ref_col_name = if reference.is_some() {
        if is_ru { "Реком." } else { "Recom." }
    } else {
        if is_ru { "Тек." } else { "Live" } 
    };
    
    // ИСПРАВЛЕНИЕ: Добавлены .to_string() для приведения всех элементов массива к типу String
    let header_cells = [
        tr("set_param", lang),
        if is_ru { "Выбранный".to_string() } else { "Selected".to_string() },
        ref_col_name.to_string(),
        tr("set_diff", lang),
    ];
    
    let header = Row::new(header_cells)
        .style(Style::default().fg(app.ui_state.get_color(&theme.accent)).add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);
        
    let mut rows = Vec::new();
    
    // Макрос для таблицы
    macro_rules! cmp_row {
        ($label:expr, $val_sel:expr, $val_ref:expr) => {
            let s_val = $val_sel;
            let r_val = $val_ref; // Option value
            
            let (r_str, diff_str, style) = if let Some(r) = r_val {
                let diff = s_val as i32 - r as i32;
                if diff == 0 {
                    (format!("{}", r), "=".to_string(), Style::default().fg(Color::DarkGray))
                } else {
                    (format!("{}", r), format!("{:+.0}", diff), Style::default().fg(Color::Yellow))
                }
            } else {
                ("-".to_string(), "".to_string(), Style::default())
            };
            
            rows.push(Row::new(vec![
                Cell::from(format!("  {}", $label)),
                Cell::from(format!("{}", s_val)).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(r_str),
                Cell::from(diff_str).style(style),
            ]));
        };
        // Signed version
        ($label:expr, $val_sel:expr, $val_ref:expr, "signed") => {
            let s_val = $val_sel;
            let r_val = $val_ref;
            
            let (r_str, diff_str, style) = if let Some(r) = r_val {
                let diff = s_val - r;
                if diff == 0 {
                    (format!("{}", r), "=".to_string(), Style::default().fg(Color::DarkGray))
                } else {
                    (format!("{}", r), format!("{:+.0}", diff), Style::default().fg(Color::Yellow))
                }
            } else {
                ("-".to_string(), "".to_string(), Style::default())
            };
            
            rows.push(Row::new(vec![
                Cell::from(format!("  {}", $label)),
                Cell::from(format!("{}", s_val)).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(r_str),
                Cell::from(diff_str).style(style),
            ]));
        };
    }
    
    macro_rules! add_header {
        ($label:expr) => {
             rows.push(Row::new(vec![
                Cell::from($label).style(Style::default().fg(app.ui_state.get_color(&theme.highlight)).add_modifier(Modifier::BOLD)),
                Cell::from(""), Cell::from(""), Cell::from(""),
            ]));
        }
    }

    add_header!(tr("grp_gen", lang));
    cmp_row!(tr("p_fuel", lang), selected.fuel, reference.map(|a| a.fuel));
    cmp_row!(tr("p_bias", lang), selected.brake_bias, reference.map(|a| a.brake_bias));
    cmp_row!(tr("p_limiter", lang), selected.engine_limiter, reference.map(|a| a.engine_limiter));
    
    add_header!(tr("grp_tyres", lang));
    cmp_row!(format!("{} FL", tr("p_press", lang)), selected.pressure_lf, reference.map(|a| a.pressure_lf));
    cmp_row!(format!("{} FR", tr("p_press", lang)), selected.pressure_rf, reference.map(|a| a.pressure_rf));
    cmp_row!(format!("{} RL", tr("p_press", lang)), selected.pressure_lr, reference.map(|a| a.pressure_lr));
    cmp_row!(format!("{} RR", tr("p_press", lang)), selected.pressure_rr, reference.map(|a| a.pressure_rr));
    
    add_header!(tr("grp_aero", lang));
    cmp_row!(format!("{} 1", tr("p_wing", lang)), selected.wing_1, reference.map(|a| a.wing_1));
    cmp_row!(format!("{} 2", tr("p_wing", lang)), selected.wing_2, reference.map(|a| a.wing_2));
    
    add_header!(tr("grp_align", lang));
    cmp_row!(format!("{} FL", tr("p_camber", lang)), selected.camber_lf, reference.map(|a| a.camber_lf), "signed");
    cmp_row!(format!("{} FR", tr("p_camber", lang)), selected.camber_rf, reference.map(|a| a.camber_rf), "signed");
    cmp_row!(format!("{} FL", tr("p_toe", lang)), selected.toe_lf, reference.map(|a| a.toe_lf), "signed");
    cmp_row!(format!("{} FR", tr("p_toe", lang)), selected.toe_rf, reference.map(|a| a.toe_rf), "signed");
    
    add_header!(tr("grp_susp", lang));
    cmp_row!(format!("{} F", tr("p_arb", lang)), selected.arb_front, reference.map(|a| a.arb_front));
    cmp_row!(format!("{} R", tr("p_arb", lang)), selected.arb_rear, reference.map(|a| a.arb_rear));
    cmp_row!(format!("{} FL", tr("p_spring", lang)), selected.spring_lf, reference.map(|a| a.spring_lf));
    cmp_row!(format!("{} FR", tr("p_spring", lang)), selected.spring_rf, reference.map(|a| a.spring_rf));
    cmp_row!(format!("{} FL", tr("p_rod", lang)), selected.rod_length_lf, reference.map(|a| a.rod_length_lf), "signed");
    
    add_header!(tr("grp_damp", lang));
    cmp_row!(format!("{} FL", tr("p_bump", lang)), selected.damp_bump_lf, reference.map(|a| a.damp_bump_lf));
    cmp_row!(format!("{} FL", tr("p_reb", lang)), selected.damp_rebound_lf, reference.map(|a| a.damp_rebound_lf));
    
    add_header!(tr("grp_driv", lang));
    cmp_row!(tr("p_diff_p", lang), selected.diff_power, reference.map(|a| a.diff_power));
    cmp_row!(tr("p_diff_c", lang), selected.diff_coast, reference.map(|a| a.diff_coast));
    cmp_row!(tr("p_final", lang), selected.final_ratio, reference.map(|a| a.final_ratio));
    
    for (i, gear) in selected.gears.iter().enumerate() {
        let ref_gear = reference.and_then(|a| a.gears.get(i).cloned());
        cmp_row!(format!("{} {}", tr("p_gear", lang), i + 2), *gear, ref_gear);
    }

    let table = Table::new(rows, [
        Constraint::Percentage(40), 
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20)
    ])
    .header(header)
    .block(Block::default().padding(Padding::new(1, 0, 0, 0)));
        
    f.render_widget(table, area);
}