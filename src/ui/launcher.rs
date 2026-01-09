use ratatui::{prelude::*, widgets::*};
use crate::AppState;
use crate::config::Language;
use crate::ui::localization::tr; 
use crate::updater::UpdateStatus;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn render(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    
    
    let block = Block::default()
        .style(Style::default().bg(app.ui_state.get_color(&theme.background)));
    f.render_widget(block, area);

    
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), 
            Constraint::Min(0),   
            Constraint::Length(3), 
        ])
        .split(area);

    render_header(f, layout[0], app);
    render_main_content(f, layout[1], app);
    render_status_bar(f, layout[2], app);

   
    if app.show_update_success {
        render_success_popup(f, area, app);
    }
}

fn render_success_popup(f: &mut Frame, area: Rect, app: &AppState) {
    let is_ru = app.config.language == Language::Russian;
    let popup_area = center_rect(area, 40, 10);
    
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black))
        .border_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .title(if is_ru { " ОБНОВЛЕНИЕ " } else { " UPDATE " })
        .title_alignment(Alignment::Center);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            if is_ru { "УСПЕШНО ОБНОВЛЕНО!" } else { "SUCCESSFULLY UPDATED!" },
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        )),
        Line::from(format!("v{}", crate::updater::CURRENT_VERSION)),
        Line::from(""),
        Line::from(Span::styled(
            if is_ru { "Нажмите ENTER чтобы продолжить" } else { "Press ENTER to continue" },
            Style::default().fg(Color::DarkGray)
        )),
    ];
    
    let p = Paragraph::new(text).block(block).alignment(Alignment::Center);
    f.render_widget(Clear, popup_area);
    f.render_widget(p, popup_area);
}

fn render_header(f: &mut Frame, area: Rect, _app: &AppState) {
  
    let ver_str = format!("   TELEMETRY & ENGINEER TOOL v{}    ", crate::updater::CURRENT_VERSION);

    let logo_text = ["   ___   _____  __     ___  ___  ___ ".to_string(),
        "  / _ | / __/ |/ /    / _ \\/ _ \\/ _ \\".to_string(),
        " / __ |/ _/ /    /   / ___/ , _/ // /".to_string(),
        "/_/ |_/_/  /_/|_/   /_/  /_/|_|\\___/ ".to_string(),
        ver_str];

    
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    let pulse = (time / 150) % 20; 
    let color = if pulse < 10 { Color::Cyan } else { Color::LightCyan };

    let logo = Paragraph::new(logo_text.join("\n"))
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    
    let center_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(5), Constraint::Min(0)])
        .split(area)[1];

    f.render_widget(logo, center_area);
}

fn render_main_content(f: &mut Frame, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), 
            Constraint::Percentage(65), 
        ])
        .split(area);

    let menu_area = center_rect(layout[0], 34, 18);
    let info_area = layout[1].inner(&Margin { vertical: 2, horizontal: 2 });

    render_menu(f, menu_area, app);
    render_info_panel(f, info_area, app);
}

fn render_menu(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    
    let update_status = app.updater.status.lock().unwrap();
    let update_label = match *update_status {
        UpdateStatus::Downloading(pct) => format!("♻   {:.0}%", pct),
        UpdateStatus::UpdateAvailable(_) => format!("!   {}", tr("launch_upd", lang)),
        _ => format!("♻   {}", tr("launch_upd", lang)),
    };

    let menu_items = [
        format!("🚀  {}", tr("launch_start", lang)), 
        format!("⚙️   {}", tr("launch_sett", lang)), 
        match app.config.language { Language::English => "LANGUAGE: < ENGLISH >", Language::Russian => "ЯЗЫК: < РУССКИЙ >" }.to_string(), 
        format!("📚  {}", tr("launch_docs", lang)), 
        format!("👤  {}", tr("launch_cred", lang)), 
        update_label, 
        format!("❌  {}", tr("launch_exit", lang)),
    ];
    
    let sel = app.launcher_selection;
    let items: Vec<ListItem> = menu_items.iter().enumerate().map(|(i, text)| {
        let is_selected = i == sel;
        
        let mut style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(app.ui_state.get_color(&theme.highlight))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        
       
        if i == 5 {
             if let UpdateStatus::UpdateAvailable(_) = *update_status {
                 if !is_selected { style = style.fg(Color::Yellow).add_modifier(Modifier::BOLD); }
             }
        }
        
        let prefix = if is_selected { " " } else { " " };
        ListItem::new(format!("{}{}", prefix, text)).style(style)
    }).collect();
    
    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)))
            .title(tr("launch_menu_title", lang))
            .title_alignment(Alignment::Center))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    
    f.render_widget(list, area);
}

fn render_info_panel(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;
    
    
    let title = match app.launcher_selection {
        0 => tr("launch_info_title", lang),
        1 => tr("launch_conf_title", lang),
        2 => tr("launch_lang_title", lang),
        3 => tr("launch_doc_title", lang),
        4 => tr("launch_cred_title", lang),
        5 => tr("launch_upd_title", lang),
        6 => tr("launch_shut_title", lang),
        _ => tr("launch_info_title", lang),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.accent)))
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let update_status = app.updater.status.lock().unwrap();

    
    let content = match app.launcher_selection {
        0 => vec![ 
            Line::from(Span::styled(tr("launch_ready", lang), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(tr("launch_conn_desc", lang)),
            Line::from(""),
            Line::from(vec![
                Span::raw(format!("{} ", tr("launch_stat", lang))),
                if app.is_game_running { 
                    Span::styled(tr("launch_detect", lang), Style::default().fg(Color::Green)) 
                } else { 
                    Span::styled(tr("launch_wait", lang), Style::default().fg(Color::Yellow)) 
                }
            ]),
        ],
        1 => vec![ 
            Line::from(Span::styled(tr("launch_conf_title", lang), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(tr("launch_conf_desc", lang)),
        ],
        2 => vec![ 
            Line::from(Span::styled(tr("launch_lang_title", lang), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(tr("launch_lang_desc", lang)),
        ],
        3 => vec![ 
            Line::from(Span::styled(tr("launch_doc_title", lang), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(Span::styled(tr("launch_nav", lang), Style::default().add_modifier(Modifier::UNDERLINED))),
            Line::from(tr("launch_nav_desc", lang)),
            Line::from(""),
            Line::from(Span::styled(tr("launch_feat", lang), Style::default().add_modifier(Modifier::UNDERLINED))),
            Line::from(tr("launch_feat_desc", lang)),
        ],
        4 => vec![ 
            Line::from(Span::styled(tr("launch_cred_title", lang), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from("AC Pro Engineer Tool"),
            Line::from(format!("Version: {}", crate::updater::CURRENT_VERSION)),
            Line::from(""),
            Line::from(Span::styled(tr("launch_created", lang), Style::default().fg(Color::Gray))),
            Line::from(Span::styled("  ***:)", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(tr("launch_thanks", lang)),
            Line::from("  Kunos Simulazioni (Assetto Corsa)"),
            Line::from("  Rust Community (Ratatui, Serde)"),
            Line::from(""),
            Line::from("© 2026 All Rights Reserved."),
        ],
        5 => { 
            let mut lines = vec![
                Line::from(Span::styled(tr("launch_upd_title", lang), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(format!("Current Version: v{}", crate::updater::CURRENT_VERSION)), 
                Line::from(""),
            ];
            
            match *update_status {
                UpdateStatus::Idle => {
                    lines.push(Line::from(if is_ru { "Нажмите ENTER для проверки обновлений." } else { "Press ENTER to check for updates." }));
                },
                UpdateStatus::Checking => {
                    lines.push(Line::from(Span::styled(if is_ru { "Соединение с сервером..." } else { "Connecting to server..." }, Style::default().fg(Color::Yellow))));
                },
                UpdateStatus::NoUpdate => {
                    lines.push(Line::from(Span::styled(tr("launch_upd_ok", lang), Style::default().fg(Color::Green))));
                },
                UpdateStatus::UpdateAvailable(ref info) => {
                    lines.push(Line::from(Span::styled(if is_ru { format!("ДОСТУПНА ВЕРСИЯ: {}", info.version) } else { format!("NEW VERSION AVAILABLE: {}", info.version) }, Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD))));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(if is_ru { "Что нового:" } else { "Changelog:" }, Style::default().fg(Color::Gray))));
                    lines.push(Line::from(Span::styled(&info.notes, Style::default().fg(Color::White))));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(if is_ru { "Нажмите ENTER для скачивания!" } else { "Press ENTER to Download!" }, Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK))));
                },
                UpdateStatus::Downloading(pct) => {
                    lines.push(Line::from(Span::styled(if is_ru { "Скачивание обновления..." } else { "Downloading update..." }, Style::default().fg(Color::Cyan))));
                    let filled = (pct / 5.0) as usize;
                    let bar = "█".repeat(filled) + &"░".repeat(20 - filled);
                    lines.push(Line::from(Span::styled(format!("{} {:.1}%", bar, pct), Style::default().fg(Color::Cyan))));
                },
                UpdateStatus::Downloaded(_) => {
                    lines.push(Line::from(Span::styled(if is_ru { "ГОТОВО К УСТАНОВКЕ!" } else { "READY TO INSTALL!" }, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));
                    lines.push(Line::from(""));
                    lines.push(Line::from(if is_ru { "Нажмите ENTER для перезапуска..." } else { "Press ENTER to restart..." }));
                },
                UpdateStatus::Error(ref err) => {
                    lines.push(Line::from(Span::styled(if is_ru { "ОШИБКА:" } else { "ERROR:" }, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))));
                    lines.push(Line::from(Span::styled(err, Style::default().fg(Color::Red))));
                },
            }
            lines
        },
        6 => vec![ 
            Line::from(Span::styled(tr("launch_shut_title", lang), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(tr("launch_safe", lang)),
        ],
        _ => vec![],
    };

    let p = Paragraph::new(content)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(app.ui_state.get_color(&theme.text)));
        
    f.render_widget(p, inner);
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    
    let time_secs = app.last_update.elapsed().as_secs();
    let (msg, color) = if time_secs < 2 {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(app.last_update.elapsed().as_millis() / 100 % 10) as usize];
        (format!("{} Connecting...", spinner), Color::Yellow)
    } else {
        (tr("launch_on", lang), Color::Green)
    };

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let status = Paragraph::new(msg).style(Style::default().fg(color));
    
    let copyright = Paragraph::new(tr("launch_hint", lang))
        .alignment(Alignment::Right)
        .style(Style::default().fg(Color::DarkGray));

    f.render_widget(status, layout[0]);
    f.render_widget(copyright, layout[1]);
    
    let border = Block::default().borders(Borders::TOP).border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    f.render_widget(border, area);
}

fn center_rect(r: Rect, w: u16, h: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(h)) / 2),
            Constraint::Length(h),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((r.width.saturating_sub(w)) / 2),
            Constraint::Length(w),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1]
}