use crate::ui::localization::tr;
use crate::AppState;
use ac_core::setup_manager::CarSetup;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;

    let main_block = Block::default()
        .title(tr("tab_setup", lang))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = main_block.inner(area);
    f.render_widget(main_block, area);

    let fetching = !*app
        .setup_manager
        .server_fetch_done
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if fetching {
        let tick = *app
            .setup_manager
            .loading_tick
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = frames[tick % frames.len()];
        let spinner_text = format!(" {} Loading... ", frame);
        let spinner = Paragraph::new(spinner_text)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Right);

        let spinner_area = Rect {
            x: area.x + area.width - 15,
            y: area.y,
            width: 14,
            height: 1,
        };
        f.render_widget(spinner, spinner_area);
    }

    let is_browser = *app
        .setup_manager
        .browser_active
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let current_car = app
        .setup_manager
        .current_car
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();

    let hint_text = if is_browser {
        if *lang == ac_core::config::Language::Russian {
            "БРАУЗЕР: Стрелки - Навигация | ENTER - Выбор | 'D' - Скачать | 'B' - Назад | PgUp/PgDn - Скролл"
        } else {
            "BROWSER: Arrows - Navigate | ENTER - Select | 'D' - Download | 'B' - Return | PgUp/PgDn - Scroll"
        }
    } else if *lang == ac_core::config::Language::Russian {
        "LIVE: 'B' - База Сетапов | 'D' - Скачать | PgUp/PgDn - Скролл деталей"
    } else {
        "LIVE: 'B' - Online Database | 'D' - Download | PgUp/PgDn - Scroll Details"
    };

    let hint_area = Rect {
        x: area.x + 2,
        y: area.y + area.height - 1,
        width: area.width - 4,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(hint_text).style(Style::default().fg(Color::Yellow)),
        hint_area,
    );

    let content_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height - 1,
    };

    if is_browser {
        render_browser_mode(f, content_area, app);
    } else {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(content_area);
        let setups = app.setup_manager.get_setups();
        let best_setup_idx = app.setup_manager.get_best_match_index();
        let list_title = format!("{} ({})", tr("set_list", lang), current_car);

        render_setup_list_classic(f, layout[0], app, &setups, best_setup_idx, &list_title);

        if let Some(selected_idx) = app.ui_state.setup_list_state.selected() {
            if selected_idx < setups.len() {
                let selected_setup = &setups[selected_idx];
                let reference_setup = if let Some(best_idx) = best_setup_idx {
                    if best_idx != selected_idx {
                        setups.get(best_idx)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let right_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Max(7), Constraint::Min(0)])
                    .split(layout[1]);
                render_header_block(f, right_layout[0], app, selected_setup, reference_setup);
                render_comparison_table(f, right_layout[1], app, selected_setup, reference_setup);
            }
        } else if !setups.is_empty() {
            let selected_setup = &setups[0];
            let right_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Max(7), Constraint::Min(0)])
                .split(layout[1]);
            render_header_block(f, right_layout[0], app, selected_setup, None);
            render_comparison_table(f, right_layout[1], app, selected_setup, None);
        } else {
            let no_data = Paragraph::new(tr("set_no_file", lang))
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::LEFT));
            f.render_widget(no_data, layout[1]);
        }
    }
}

fn render_browser_mode(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(50),
        ])
        .split(area);

    let focus_col = *app
        .setup_manager
        .browser_focus_col
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let theme = &app.ui_state.theme;

    let manifest = app.setup_manager.get_manifest();

    if manifest.is_empty() {
        f.render_widget(
            Paragraph::new("Loading...")
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    }

    let car_idx = *app
        .setup_manager
        .browser_car_idx
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let car_items: Vec<ListItem<'_>> = manifest
        .iter()
        .map(|m| ListItem::new(format!("{} ({})", m.id, m.count)))
        .collect();
    let car_block = Block::default()
        .title(" 1. CARS ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focus_col == 0 {
            Color::Yellow
        } else {
            Color::Gray
        }));
    let mut car_state = ListState::default();
    car_state.select(Some(car_idx));
    f.render_stateful_widget(
        List::new(car_items).block(car_block).highlight_style(
            Style::default()
                .bg(app.ui_state.get_color(&theme.highlight))
                .fg(Color::Black),
        ),
        layout[0],
        &mut car_state,
    );

    let setups = app.setup_manager.get_browser_setups();

    let setup_idx = *app
        .setup_manager
        .browser_setup_idx
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let target_car = app.setup_manager.get_browser_target_car();

    let setup_items: Vec<ListItem<'_>> = setups
        .iter()
        .map(|s| {
            let is_installed = app.setup_manager.is_installed(s, &target_car);
            let icon = if is_installed { "✓" } else { " " };
            let color = if is_installed {
                Color::Green
            } else {
                Color::White
            };
            let author_str = if !s.author.is_empty() {
                format!(" @{}", s.author)
            } else {
                "".to_string()
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", icon),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}{}", s.name, author_str),
                    Style::default().fg(color),
                ),
            ]))
        })
        .collect();

    let setup_block = Block::default()
        .title(" 2. SETUPS ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focus_col == 1 {
            Color::Yellow
        } else {
            Color::Gray
        }));
    let mut setup_state = ListState::default();
    setup_state.select(Some(setup_idx));
    f.render_stateful_widget(
        List::new(setup_items).block(setup_block).highlight_style(
            Style::default()
                .bg(app.ui_state.get_color(&theme.highlight))
                .fg(Color::Black),
        ),
        layout[1],
        &mut setup_state,
    );

    if !setups.is_empty() && setup_idx < setups.len() {
        let setup = &setups[setup_idx];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(7), Constraint::Min(0)])
            .split(layout[2]);
        render_header_block(f, chunks[0], app, setup, None);
        render_comparison_table(f, chunks[1], app, setup, None);
    } else {
        let block = Block::default().borders(Borders::ALL).title(" DETAILS ");
        f.render_widget(
            Paragraph::new("Select a car and setup...").block(block),
            layout[2],
        );
    }
}

fn render_setup_list_classic(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    setups: &[CarSetup],
    best_idx: Option<usize>,
    title: &str,
) {
    let theme = &app.ui_state.theme;
    let block = Block::default()
        .title(title)
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    let items: Vec<ListItem<'_>> = setups
        .iter()
        .enumerate()
        .map(|(i, setup)| {
            let is_best = Some(i) == best_idx;
            let (icon, color, name_style) = if setup.is_remote {
                ("☁", Color::Cyan, Style::default().fg(Color::Cyan))
            } else if is_best {
                (
                    "★",
                    Color::Green,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    "•",
                    app.ui_state.get_color(&theme.text),
                    Style::default().fg(app.ui_state.get_color(&theme.text)),
                )
            };
            let author_suffix = if !setup.author.is_empty()
                && setup.author != "Local"
                && setup.author != "Server"
            {
                format!(" @{}", setup.author)
            } else {
                "".to_string()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(setup.name.to_string(), name_style),
                Span::styled(
                    format!(" ({}){}", setup.source, author_suffix),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let mut state = app.ui_state.setup_list_state.clone();
    f.render_stateful_widget(
        List::new(items).block(block).highlight_style(
            Style::default()
                .bg(app.ui_state.get_color(&theme.highlight))
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        area,
        &mut state,
    );
}

fn render_header_block(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    selected: &CarSetup,
    best: Option<&CarSetup>,
) {
    let lang = &app.config.language;
    let is_ru = *lang == ac_core::config::Language::Russian;

    let status = app.setup_manager.get_status_message();
    let has_status = !status.is_empty();
    let status_line = if has_status {
        let color = if status.contains("Err") {
            Color::Red
        } else {
            Color::Green
        };
        Line::from(Span::styled(
            status,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from("")
    };

    let is_browser = *app
        .setup_manager
        .browser_active
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let target_car = if is_browser {
        app.setup_manager.get_browser_target_car()
    } else {
        app.setup_manager
            .current_car
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    };
    let is_installed = app.setup_manager.is_installed(selected, &target_car);

    if selected.is_remote {
        let download_text = if is_installed {
            if is_ru {
                "✓ УСТАНОВЛЕНО (D для обновления)"
            } else {
                "✓ INSTALLED (D to overwrite)"
            }
        } else if is_ru {
            "Нажми 'D' для СКАЧИВАНИЯ"
        } else {
            "Press 'D' to DOWNLOAD"
        };
        let dl_color = if is_installed {
            Color::Green
        } else {
            Color::Yellow
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    tr("set_server_title", lang),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(": {}", selected.name)),
            ]),
            Line::from(vec![
                Span::raw("Author: "),
                Span::styled(&selected.author, Style::default().fg(Color::White)),
            ]),
        ];

        if !selected.notes.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Note: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    &selected.notes,
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        if !selected.credits.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    if is_ru { "Credits: " } else { "Credits: " },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    &selected.credits,
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        lines.extend(vec![
            Line::from(vec![Span::styled(
                download_text,
                Style::default().fg(dl_color).add_modifier(Modifier::BOLD),
            )]),
            status_line,
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        f.render_widget(
            Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let (block_color, title, mut lines) = if let Some(best_setup) = best {
        let mut advice_lines = vec![Line::from(vec![
            Span::styled(
                if is_ru {
                    "⚠ СОВЕТ: "
                } else {
                    "⚠ ADVICE: "
                },
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(if is_ru {
                format!("Рекомендуется '{}'. Отличия:", best_setup.name)
            } else {
                format!("Recommended: '{}'. Differences:", best_setup.name)
            }),
        ])];

        if !selected.notes.is_empty() {
            advice_lines.push(Line::from(vec![
                Span::styled("Note: ", Style::default().fg(Color::Yellow)),
                Span::styled(&selected.notes, Style::default().fg(Color::Gray)),
            ]));
        }

        if !best_setup.credits.is_empty() {
            advice_lines.push(Line::from(vec![
                Span::styled("Credits: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    &best_setup.credits,
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        let diffs = app.engineer.compare_setups_advice(selected, best_setup);
        for diff in diffs {
            advice_lines.push(Line::from(vec![
                Span::raw(" • "),
                Span::styled(diff, Style::default().fg(Color::White)),
            ]));
        }
        (
            Color::Yellow,
            if is_ru {
                "АНАЛИЗ СЕТАПА"
            } else {
                "SETUP ANALYSIS"
            },
            advice_lines,
        )
    } else {
        let mut verdict_lines = vec![Line::from(vec![Span::styled(
            if is_ru {
                "✓ ОТЛИЧНЫЙ ВЫБОР"
            } else {
                "✓ EXCELLENT CHOICE"
            },
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )])];

        if !selected.notes.is_empty() {
            verdict_lines.push(Line::from(vec![
                Span::styled("Note: ", Style::default().fg(Color::Yellow)),
                Span::styled(&selected.notes, Style::default().fg(Color::Gray)),
            ]));
        }

        verdict_lines.push(Line::from(if is_ru {
            "Этот сетап подходит."
        } else {
            "This setup is a good match."
        }));

        if !selected.credits.is_empty() {
            verdict_lines.push(Line::from(vec![
                Span::styled("Credits: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    &selected.credits,
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
        }

        (
            Color::Green,
            if is_ru {
                "ВЕРДИКТ ИНЖЕНЕРА"
            } else {
                "ENGINEER VERDICT"
            },
            verdict_lines,
        )
    };

    if has_status {
        lines.push(Line::from(""));
        lines.push(status_line);
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(block_color));
    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_comparison_table(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    selected: &CarSetup,
    reference: Option<&CarSetup>,
) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let scroll_offset = *app
        .setup_manager
        .details_scroll
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if let Some(target) = reference {
        let diffs = selected.generate_diff(target);

        let header = Row::new(vec![
            if is_ru {
                "Параметр"
            } else {
                "Parameter"
            },
            if is_ru { "Текущий" } else { "Current" },
            if is_ru { "Эталон" } else { "Reference" },
            if is_ru { "Разница" } else { "Diff" },
        ])
        .style(
            Style::default()
                .fg(app.ui_state.get_color(&theme.accent))
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

        if diffs.is_empty() {
            let p = Paragraph::new(if is_ru {
                "Сетапы полностью идентичны!"
            } else {
                "Setups are completely identical!"
            })
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Green));
            f.render_widget(p, area);
            return;
        }

        let mut diff_rows = vec![];
        for d in diffs {
            let diff_str = if d.diff > 0.0 {
                format!("+{:.1}", d.diff)
            } else {
                format!("{:.1}", d.diff)
            };
            let color = if d.diff > 0.0 {
                Color::Red
            } else {
                Color::Green
            };

            diff_rows.push(Row::new(vec![
                Cell::from(d.name),
                Cell::from(format!("{:.1}", d.current)),
                Cell::from(format!("{:.1}", d.reference)),
                Cell::from(diff_str).style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ]));
        }

        let total_rows = diff_rows.len();
        let start = if scroll_offset >= total_rows {
            total_rows.saturating_sub(1)
        } else {
            scroll_offset
        };
        let visible_rows = diff_rows.into_iter().skip(start).collect::<Vec<Row<'_>>>();

        let table = Table::new(
            visible_rows,
            [
                Constraint::Percentage(40),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
            ],
        )
        .header(header)
        .block(Block::default().padding(Padding::new(1, 0, 0, 0)));
        f.render_widget(table, area);
    } else {
        let p = Paragraph::new(if is_ru {
            "Для сравнения выберите сетап в базе."
        } else {
            "Select a setup to see differences."
        })
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
    }
}
