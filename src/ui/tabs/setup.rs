use ratatui::{prelude::*, widgets::*};
use crate::AppState;
use crate::setup_manager::CarSetup;
use crate::ui::localization::tr;

pub fn render(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let setups = app.setup_manager.get_setups();
    
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
    render_setup_list(f, layout[0], app, &setups);
    
    // --- Правая панель: Сравнение и Инженер ---
    if let Some(selected_idx) = app.ui_state.setup_list_state.selected() {
        if selected_idx < setups.len() {
            let selected_setup = &setups[selected_idx];
            let active_setup = app.setup_manager.get_active_setup();
            
            // Вертикальный макет для правой части: Инфо инженера + Таблица
            let right_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4), // Блок инженера
                    Constraint::Min(0),    // Таблица
                ])
                .split(layout[1]);
                
            render_engineer_verdict(f, right_layout[0], app, selected_setup, &app.session_info.track_name);
            render_comparison_table(f, right_layout[1], app, selected_setup, active_setup.as_ref());
        }
    } else if !setups.is_empty() {
        // Хак: Если ничего не выбрано, но список не пуст, рендерим первый элемент (при старте)
        let selected_setup = &setups[0];
        let active_setup = app.setup_manager.get_active_setup();
        
        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(0),
            ])
            .split(layout[1]);
            
        render_engineer_verdict(f, right_layout[0], app, selected_setup, &app.session_info.track_name);
        render_comparison_table(f, right_layout[1], app, selected_setup, active_setup.as_ref());
    } else {
        // Если файлов нет
        let no_data = Paragraph::new(tr("set_no_file", lang))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::LEFT));
        f.render_widget(no_data, layout[1]);
    }
}

fn render_setup_list(f: &mut Frame, area: Rect, app: &AppState, setups: &[CarSetup]) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let block = Block::default()
        .title(tr("set_list", lang))
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
        
    let items: Vec<ListItem> = setups.iter().map(|setup| {
        let is_track_specific = setup.source == app.session_info.track_name;
        
        let (icon, color) = if is_track_specific {
            ("★", Color::Green) // Звезда для сетапов этой трассы
        } else {
            ("•", app.ui_state.get_color(&theme.text))
        };
        
        let content = Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(color)),
            Span::styled(format!("{}", setup.name), Style::default().fg(app.ui_state.get_color(&theme.text))),
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

fn render_engineer_verdict(f: &mut Frame, area: Rect, _app: &AppState, setup: &CarSetup, track_name: &str) {
    let is_suitable = setup.source == track_name;
    let is_generic = setup.source == "Generic";
    
    // Исправлено: приводим все строки к типу String, чтобы избежать ошибки E0716 с временными значениями
    let (color, icon, text) = if is_suitable {
        (
            Color::Green, 
            "✓", 
            "РЕКОМЕНДУЕТСЯ: Этот сетап создан специально для текущей трассы.".to_string()
        )
    } else if is_generic {
        (
            Color::Yellow, 
            "⚠", 
            "БАЗОВЫЙ: Универсальный сетап. Может потребоваться адаптация передач и крыла.".to_string()
        )
    } else {
        (
            Color::Red, 
            "✕", 
            format!("НЕ ОПТИМАЛЬНО: Сетап для трассы '{}', характеристики могут не подойти.", setup.source)
        )
    };
    
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color));
        
    let content = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(format!("{} ИНЖЕНЕР: ", icon), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::raw(text),
        ])
    ])
    .block(block)
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });
    
    f.render_widget(content, area);
}

fn render_comparison_table(f: &mut Frame, area: Rect, app: &AppState, selected: &CarSetup, active: Option<&CarSetup>) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let header_cells = [
        tr("set_param", lang),
        tr("set_val", lang), // Значение выбранного файла
        "Live/Active".to_string(), // Значение активного (если есть)
        tr("set_diff", lang),
    ];
    
    let header = Row::new(header_cells)
        .style(Style::default().fg(app.ui_state.get_color(&theme.accent)).add_modifier(Modifier::BOLD))
        .height(1)
        .bottom_margin(1);
        
    let mut rows = Vec::new();
    
    // Макрос для добавления строк с автоматическим сравнением
    // Label | Selected Value | Active Value | Diff
    macro_rules! cmp_row {
        ($label:expr, $sel_val:expr, $act_val:expr) => {
            let s_val = $sel_val;
            let a_val = $act_val;
            
            let (act_str, diff_str, style) = if let Some(a) = a_val {
                let diff = s_val as i32 - a as i32;
                if diff == 0 {
                    (format!("{}", a), "-".to_string(), Style::default().fg(Color::Green))
                } else {
                    (format!("{}", a), format!("{:+.0}", diff), Style::default().fg(Color::Yellow))
                }
            } else {
                ("-".to_string(), "-".to_string(), Style::default())
            };
            
            rows.push(Row::new(vec![
                Cell::from(format!("  {}", $label)),
                Cell::from(format!("{}", s_val)).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(act_str),
                Cell::from(diff_str).style(style),
            ]));
        };
        // Версия для знаковых чисел (i32)
        ($label:expr, $sel_val:expr, $act_val:expr, "signed") => {
            let s_val = $sel_val;
            let a_val = $act_val;
            
            let (act_str, diff_str, style) = if let Some(a) = a_val {
                let diff = s_val - a;
                if diff == 0 {
                    (format!("{}", a), "-".to_string(), Style::default().fg(Color::Green))
                } else {
                    (format!("{}", a), format!("{:+.0}", diff), Style::default().fg(Color::Yellow))
                }
            } else {
                ("-".to_string(), "-".to_string(), Style::default())
            };
            
            rows.push(Row::new(vec![
                Cell::from(format!("  {}", $label)),
                Cell::from(format!("{}", s_val)).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(act_str),
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

    // --- General ---
    add_header!(tr("grp_gen", lang));
    cmp_row!(tr("p_fuel", lang), selected.fuel, active.map(|a| a.fuel));
    cmp_row!(tr("p_bias", lang), selected.brake_bias, active.map(|a| a.brake_bias));
    cmp_row!(tr("p_limiter", lang), selected.engine_limiter, active.map(|a| a.engine_limiter));
    
    // --- Tyres ---
    add_header!(tr("grp_tyres", lang));
    cmp_row!(format!("{} FL", tr("p_press", lang)), selected.pressure_lf, active.map(|a| a.pressure_lf));
    cmp_row!(format!("{} FR", tr("p_press", lang)), selected.pressure_rf, active.map(|a| a.pressure_rf));
    cmp_row!(format!("{} RL", tr("p_press", lang)), selected.pressure_lr, active.map(|a| a.pressure_lr));
    cmp_row!(format!("{} RR", tr("p_press", lang)), selected.pressure_rr, active.map(|a| a.pressure_rr));
    
    // --- Aero ---
    add_header!(tr("grp_aero", lang));
    cmp_row!(format!("{} 1", tr("p_wing", lang)), selected.wing_1, active.map(|a| a.wing_1));
    cmp_row!(format!("{} 2", tr("p_wing", lang)), selected.wing_2, active.map(|a| a.wing_2));
    
    // --- Alignment (Signed values) ---
    add_header!(tr("grp_align", lang));
    cmp_row!(format!("{} FL", tr("p_camber", lang)), selected.camber_lf, active.map(|a| a.camber_lf), "signed");
    cmp_row!(format!("{} FR", tr("p_camber", lang)), selected.camber_rf, active.map(|a| a.camber_rf), "signed");
    cmp_row!(format!("{} FL", tr("p_toe", lang)), selected.toe_lf, active.map(|a| a.toe_lf), "signed");
    cmp_row!(format!("{} FR", tr("p_toe", lang)), selected.toe_rf, active.map(|a| a.toe_rf), "signed");
    
    // --- Suspension ---
    add_header!(tr("grp_susp", lang));
    cmp_row!(format!("{} F", tr("p_arb", lang)), selected.arb_front, active.map(|a| a.arb_front));
    cmp_row!(format!("{} R", tr("p_arb", lang)), selected.arb_rear, active.map(|a| a.arb_rear));
    cmp_row!(format!("{} FL", tr("p_spring", lang)), selected.spring_lf, active.map(|a| a.spring_lf));
    cmp_row!(format!("{} FR", tr("p_spring", lang)), selected.spring_rf, active.map(|a| a.spring_rf));
    cmp_row!(format!("{} FL", tr("p_rod", lang)), selected.rod_length_lf, active.map(|a| a.rod_length_lf), "signed");
    
    // --- Dampers ---
    add_header!(tr("grp_damp", lang));
    cmp_row!(format!("{} FL", tr("p_bump", lang)), selected.damp_bump_lf, active.map(|a| a.damp_bump_lf));
    cmp_row!(format!("{} FL", tr("p_reb", lang)), selected.damp_rebound_lf, active.map(|a| a.damp_rebound_lf));
    
    // --- Drivetrain ---
    add_header!(tr("grp_driv", lang));
    cmp_row!(tr("p_diff_p", lang), selected.diff_power, active.map(|a| a.diff_power));
    cmp_row!(tr("p_diff_c", lang), selected.diff_coast, active.map(|a| a.diff_coast));
    cmp_row!(tr("p_final", lang), selected.final_ratio, active.map(|a| a.final_ratio));
    
    for (i, gear) in selected.gears.iter().enumerate() {
        let active_gear = active.and_then(|a| a.gears.get(i).cloned());
        // Исправлено: разыменовываем gear (*gear), так как итератор дает ссылку
        cmp_row!(format!("{} {}", tr("p_gear", lang), i + 2), *gear, active_gear);
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