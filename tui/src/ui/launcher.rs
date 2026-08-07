use crate::ui::localization::tr;
use crate::{AppState, OverlayOnboarding};
use ac_core::config::Language;
use ac_core::updater::UpdateStatus;
use ratatui::{prelude::*, widgets::*};
use std::sync::atomic::AtomicBool;

pub static SHOW_REVIEW_BANNER: AtomicBool = AtomicBool::new(true);

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let block =
        Block::default().style(Style::default().bg(app.ui_state.get_color(&theme.background)));
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

    if app.show_first_run_prompt {
        render_first_run_popup(f, area, app);
    } else if app.onboarding == OverlayOnboarding::Offer {
        render_overlay_offer(f, area, app);
    } else if app.onboarding == OverlayOnboarding::Tips {
        render_overlay_tips(f, area, app);
    } else if app.show_overlay_card {
        render_overlay_card(f, area, app);
    }
}

/// The offer, made once: there is an overlay, and it can be in the game in a
/// second. Asked here rather than buried in Settings, because a feature nobody
/// is told about is a feature nobody has.
fn render_overlay_offer(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let popup_area = center_rect(area, 70, 16);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .title(" IN-GAME OVERLAY ")
        .title_alignment(Alignment::Center);

    let dim = Style::default().fg(Color::DarkGray);
    let white = Style::default().fg(Color::White);

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "There is a panel that draws all of this inside the car.",
            white,
        )),
        Line::from(Span::styled(
            "Speed, revs, four corners, fuel, lap times and the engineer's",
            dim,
        )),
        Line::from(Span::styled(
            "advice — on the windscreen, while you drive.",
            dim,
        )),
        Line::from(""),
    ];

    match app.overlay_report.game_root.as_ref() {
        Some(_) if app.overlay_report.csp_present => lines.push(Line::from(Span::styled(
            "Assetto Corsa and CSP are both here. Shall I install it?",
            Style::default().fg(Color::Green),
        ))),
        Some(_) => lines.push(Line::from(Span::styled(
            "Assetto Corsa is here, but CSP is not — the panel needs it.",
            Style::default().fg(Color::Yellow),
        ))),
        None => lines.push(Line::from(Span::styled(
            "No Assetto Corsa found yet — set the path in Settings first.",
            Style::default().fg(Color::Red),
        ))),
    }

    lines.push(Line::from(""));

    let yes = if app.overlay_card_selection == 0 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let no = if app.overlay_card_selection == 1 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    lines.push(Line::from(vec![
        Span::raw("      "),
        Span::styled(" [ YES, INSTALL IT ] ", yes),
        Span::raw("     "),
        Span::styled(" [ NOT NOW ] ", no),
    ]));
    lines.push(Line::from(Span::styled(
        "      installed later with [I] in Settings → OVERLAY, and removed",
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "      with [U] whenever you like — nothing else in the game is touched",
        dim,
    )));

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left),
        popup_area,
    );
}

/// What to do with it now that it is installed. Six lines, once.
fn render_overlay_tips(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let popup_area = center_rect(area, 72, 17);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().fg(Color::Green).bg(Color::Black))
        .title(" THE OVERLAY IS IN THE GAME ")
        .title_alignment(Alignment::Center);

    let dim = Style::default().fg(Color::DarkGray);
    let key = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let white = Style::default().fg(Color::White);

    let tip = |k: &'static str, text: &'static str| {
        Line::from(vec![
            Span::styled(format!("  {k:<22}"), key),
            Span::styled(text, white),
        ])
    };

    let mut lines = vec![Line::from("")];
    if !app.overlay_install_status.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("  {}", app.overlay_install_status),
            Style::default().fg(Color::Green),
        )));
        lines.push(Line::from(""));
    }

    lines.push(tip(
        "in game",
        "open it from CSP's app sidebar, on the right",
    ));
    lines.push(tip(
        "three windows",
        "the panel, the advice, and the settings",
    ));
    lines.push(tip(
        "the gear",
        "opens the settings on the panel's title bar",
    ));
    lines.push(tip("first thing", "Look → Screen, pick your resolution"));
    lines.push(tip("this app", "keep it running — the panel reads from it"));
    lines.push(tip(
        "Settings → [F]",
        "choose what the panel is allowed to show",
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Everything in the panel can be switched off. Nothing is required.",
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "  Remove it any time with [U] in Settings → OVERLAY. Only its own",
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "  folder goes, and your settings stay — putting it back finds them.",
        dim,
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "                     [ ENTER TO CONTINUE ]",
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left),
        popup_area,
    );
}

/// What was found in the game folder, and what the overlay's state is.
///
/// The application installs the panel at startup on its own; this says so out
/// loud, because an overlay that silently did or did not appear is the thing
/// people spend an evening on before finding out CSP was never installed.
fn render_overlay_card(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let report = &app.overlay_report;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .title(" IN-GAME OVERLAY ")
        .title_alignment(Alignment::Center);

    let dim = Style::default().fg(Color::DarkGray);
    let good = Style::default().fg(Color::Green);
    let bad = Style::default().fg(Color::Red);
    let warn = Style::default().fg(Color::Yellow);

    let short = |path: &std::path::Path| {
        let text = path.display().to_string();
        if text.len() > 52 {
            format!("…{}", &text[text.len() - 51..])
        } else {
            text
        }
    };

    let mut lines = vec![Line::from("")];

    match report.game_root.as_ref() {
        Some(root) => lines.push(Line::from(vec![
            Span::styled("game     ", dim),
            Span::styled(short(root), good),
        ])),
        None => lines.push(Line::from(vec![
            Span::styled("game     ", dim),
            Span::styled("not found — set the path in Settings", bad),
        ])),
    }

    lines.push(Line::from(vec![
        Span::styled("CSP      ", dim),
        if report.csp_present {
            Span::styled("installed", good)
        } else {
            Span::styled("missing — the panel cannot appear without it", warn)
        },
    ]));

    if let Some(path) = report.app_path.as_ref() {
        lines.push(Line::from(vec![
            Span::styled("panel    ", dim),
            Span::styled(short(path), Style::default().fg(Color::Gray)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("files    ", dim),
        if report.current {
            Span::styled("up to date", good)
        } else {
            Span::styled("not installed — press ENTER", warn)
        },
    ]));

    // Three pieces have to agree on the shape of a frame. All three are named
    // here, because "which one is old" is the question, and the answer used to
    // require reading files in two directories and a Wine prefix.
    let expected = ac_core::overlay::frame::OVERLAY_VERSION;
    lines.push(Line::from(vec![
        Span::styled("frame    ", dim),
        match report.panel_version {
            Some(version) if version == expected => {
                Span::styled(format!("v{version}, matching"), good)
            }
            Some(version) => Span::styled(
                format!("panel speaks v{version}, this app writes v{expected} — press ENTER"),
                bad,
            ),
            None => Span::styled(format!("v{expected} from this app"), dim),
        },
    ]));

    // The release, not the frame number. Most releases leave the struct alone,
    // so a matching frame version says nothing about how old the panel is.
    let app_version = ac_core::updater::CURRENT_VERSION;
    lines.push(Line::from(vec![
        Span::styled("release  ", dim),
        match report.panel_release.as_deref() {
            Some(panel) if panel == app_version => {
                Span::styled(format!("app and panel both v{panel}"), good)
            }
            Some(panel) => Span::styled(
                format!("app v{app_version}, panel v{panel} — press ENTER to refresh it"),
                warn,
            ),
            None => Span::styled(format!("app v{app_version}, no panel installed"), dim),
        },
    ]));

    lines.push(Line::from(vec![
        Span::styled("bridge   ", dim),
        bridge_span(&app.bridge_status, good, warn, bad, dim),
    ]));

    // Found by the startup check, which looks without downloading. Shown
    // whether or not the bridge in place works: a newer one is worth knowing
    // about before it becomes the reason nothing appears.
    if let Some(offer) = app.bridge_offer() {
        lines.push(Line::from(vec![
            Span::styled("update   ", dim),
            Span::styled(
                format!(
                    "shm-bridge v{} is published — press B to fetch it",
                    offer.version
                ),
                Style::default().fg(Color::Cyan),
            ),
        ]));
    }

    if !app.bridge_fetch_status.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("         {}", app.bridge_fetch_status),
            Style::default().fg(Color::White),
        )));
    }

    if !app.overlay_install_status.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            app.overlay_install_status.clone(),
            Style::default().fg(Color::White),
        )));
    }

    lines.push(Line::from(""));

    let install_style = if app.overlay_card_selection == 0 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let close_style = if app.overlay_card_selection == 1 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // The rows read as a table, so they are left-aligned; the buttons are
    // centred by hand rather than by centring the whole card and leaving
    // "CSP installed" floating in the middle of it.
    lines.push(Line::from(vec![
        Span::raw("        "),
        Span::styled(" [ INSTALL INTO THE GAME ] ", install_style),
        Span::raw("   "),
        Span::styled(" [ CLOSE ] ", close_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw("            "),
        Span::styled("D — do not show this at startup", dim),
    ]));
    // Only where a bridge is a thing. On Windows the application creates the
    // named mapping itself, and offering to download one would send people
    // looking for a component they do not have.
    if !matches!(
        app.bridge_status,
        ac_core::overlay::bridge::BridgeStatus::NotRequired
    ) {
        lines.push(Line::from(vec![
            Span::raw("            "),
            Span::styled("B — fetch the published shm-bridge.exe", dim),
        ]));
    }

    // Sized to what there is to say, rather than to a number that was right
    // when the card had five rows. A bridge complaint names two byte counts and
    // a remedy, and a fixed 66x15 clipped it to the half without the remedy.
    //
    // Wrapped as well as measured: the paths and the GitHub errors in here are
    // arbitrary length, and a wrapped sentence is worth more than a tidy box.
    let width = 78.min(area.width);
    let wrapped: usize = lines
        .iter()
        .map(|line| {
            let cells = line.width().max(1);
            cells.div_ceil(width.saturating_sub(2).max(1) as usize)
        })
        .sum();
    let height = (wrapped as u16 + 2).min(area.height);

    let popup_area = center_rect(area, width, height);
    f.render_widget(Clear, popup_area);

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);
    f.render_widget(p, popup_area);
}

/// One line about the bridge, in the colour that says how worried to be.
///
/// `Behind` is yellow rather than red on purpose: a bridge from another release
/// that maps the right bytes under the right name works, and colouring it as a
/// fault is how a check stops being read.
fn bridge_span<'a>(
    status: &ac_core::overlay::bridge::BridgeStatus,
    good: Style,
    warn: Style,
    bad: Style,
    dim: Style,
) -> Span<'a> {
    use ac_core::overlay::bridge::BridgeStatus;

    match status {
        BridgeStatus::NotRequired => Span::styled("not needed — Windows maps this directly", dim),
        BridgeStatus::NotRunning => Span::styled(
            "not running — start shm-bridge.exe in the Proton prefix",
            warn,
        ),
        // Red, not yellow: this one is running and the overlay still cannot
        // work, and telling the driver to start it sends them to start the
        // same broken bridge again.
        BridgeStatus::Unannounced => Span::styled(
            "running but too old to serve the overlay — press B, or rebuild it",
            bad,
        ),
        BridgeStatus::Unreadable(why) => Span::styled(format!("cannot be read: {why}"), warn),
        BridgeStatus::Incompatible { info, complaint } => Span::styled(
            format!("v{} {} — press B", info.version, complaint.describe()),
            bad,
        ),
        BridgeStatus::Behind {
            info,
            expected_version,
        } => Span::styled(
            format!(
                "v{} running, this app is v{expected_version} — works, B to update",
                info.version
            ),
            warn,
        ),
        BridgeStatus::Current(info) => {
            Span::styled(format!("v{} running, matching", info.version), good)
        }
    }
}

fn render_first_run_popup(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let popup_area = center_rect(area, 50, 12);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .title(" WELCOME TO AC PRO ENGINEER ")
        .title_alignment(Alignment::Center);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "This is a professional telemetry and setup tool.",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Would you like to open the interactive guide to learn",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::styled(
            "how to read the data and use hotkeys?",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(""),
    ];

    let yes_style = if app.first_run_selection == 0 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let no_style = if app.first_run_selection == 1 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let buttons = Line::from(vec![
        Span::styled(" [ YES, OPEN GUIDE ] ", yes_style),
        Span::raw("      "),
        Span::styled(" [ NO, I'M A PRO ] ", no_style),
    ]);
    let mut content = text;
    content.push(buttons);

    let p = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(p, popup_area);
}

fn render_success_popup(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let is_ru = app.config.language == Language::Russian;
    let popup_area = center_rect(area, 40, 10);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(Color::Black))
        .border_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
        .title(if is_ru {
            " ОБНОВЛЕНИЕ "
        } else {
            " UPDATE "
        })
        .title_alignment(Alignment::Center);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            if is_ru {
                "УСПЕШНО ОБНОВЛЕНО!"
            } else {
                "SUCCESSFULLY UPDATED!"
            },
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("v{}", ac_core::updater::CURRENT_VERSION)),
        Line::from(""),
        Line::from(Span::styled(
            if is_ru {
                "Нажмите ENTER чтобы продолжить"
            } else {
                "Press ENTER to continue"
            },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(Clear, popup_area);
    f.render_widget(p, popup_area);
}

fn render_header(f: &mut Frame<'_>, area: Rect, _app: &AppState) {
    let ver_str = format!(
        "   TELEMETRY & ENGINEER TOOL v{}    ",
        ac_core::updater::CURRENT_VERSION
    );
    let logo_text = [
        "   ___   _____  __     ___  ___  ___ ",
        "  / _ | / __/ |/ /    / _ \\/ _ \\/ _ \\",
        " / __ |/ _/ /    /   / ___/ , _/ // /",
        "/_/ |_/_/  /_/|_/   /_/  /_/|_|\\___/ ",
        ver_str.as_str(),
    ];

    let logo = Paragraph::new(logo_text.join("\n"))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    let center_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .split(area)[1];
    f.render_widget(logo, center_area);
}

fn render_main_content(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let mut main_area = area;

    if !app.config.review_banner_hidden {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);
        render_review_banner(f, chunks[0], app);
        main_area = chunks[1];
    }

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(main_area);

    let menu_area = center_rect(layout[0], 36, 18);
    let info_area = layout[1].inner(&Margin {
        vertical: 2,
        horizontal: 2,
    });

    render_menu(f, menu_area, app);
    render_info_panel(f, info_area, app);
}

fn render_review_banner(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let is_ru = app.config.language == Language::Russian;
    let text = if is_ru {
        "⭐ Это Open Source проект. Ваш отзыв помогает нам расти!"
    } else {
        "⭐ This is an Open Source project. Your review helps us grow!"
    };
    let hint = if is_ru {
        "[O] Оставить отзыв  [H] Скрыть навсегда"
    } else {
        "[O] Leave Review  [H] Hide Forever"
    };

    let content = vec![Line::from(vec![
        Span::styled(
            text,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(hint, Style::default().fg(Color::DarkGray)),
    ])];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(Color::Black));

    let p = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_menu(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;
    let update_status = app.updater.status.lock().unwrap_or_else(|e| e.into_inner());

    let update_label = match *update_status {
        UpdateStatus::Downloading(pct) => format!(
            "♻   {}: {:.0}%",
            if is_ru {
                "Скачивание"
            } else {
                "Downloading"
            },
            pct
        ),
        UpdateStatus::UpdateAvailable => format!(
            "🔥  {}!",
            if is_ru {
                "ДОСТУПНО"
            } else {
                "AVAILABLE"
            }
        ),
        UpdateStatus::Checking => format!(
            "⏳  {}",
            if is_ru {
                "Проверка..."
            } else {
                "Checking..."
            }
        ),
        UpdateStatus::NoUpdate => format!(
            "✅  {}",
            if is_ru {
                "Версии & Откат"
            } else {
                "Versions & Rollback"
            }
        ),
        UpdateStatus::Error(_) => format!(
            "❌  {}",
            if is_ru {
                "Ошибка сети"
            } else {
                "Net Error"
            }
        ),
        _ => format!("♻   {}", tr("launch_upd", lang)),
    };

    let menu_items = [
        format!(
            "🖥️  {}",
            if is_ru {
                "ЗАПУСК (ТЕРМИНАЛ)"
            } else {
                "START (TERMINAL TUI)"
            }
        ),
        format!("⚙️   {}", tr("launch_sett", lang)),
        match app.config.language {
            Language::English => "LANGUAGE: < ENGLISH >",
            Language::Russian => "ЯЗЫК: < РУССКИЙ >",
        }
        .to_string(),
        format!("📚  {}", tr("launch_docs", lang)),
        format!("👤  {}", tr("launch_cred", lang)),
        update_label,
        format!("❌  {}", tr("launch_exit", lang)),
    ];

    let sel = app.launcher_selection;
    let items: Vec<ListItem<'_>> = menu_items
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let is_selected = i == sel;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(app.ui_state.get_color(&theme.highlight))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            if i == 5
                && let UpdateStatus::UpdateAvailable = *update_status
            {
                if is_selected {
                    return ListItem::new(format!("  {}", text)).style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    );
                } else {
                    return ListItem::new(format!("  {}", text)).style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }

            let prefix = if is_selected { ">>" } else { "  " };
            ListItem::new(format!("{} {}", prefix, text)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)))
            .title(tr("launch_menu_title", lang))
            .title_alignment(Alignment::Center),
    );

    f.render_widget(list, area);
}

fn render_info_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;
    let update_status = app.updater.status.lock().unwrap_or_else(|e| e.into_inner());

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
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.accent)))
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Read from state rather than scanning: `tick` refreshes this, and the
    // scan behind it walks every process on the system.
    let actual_running = app.is_game_running;

    let content = match app.launcher_selection {
        0 => vec![
            Line::from(Span::styled(
                "TERMINAL MODE (TUI)",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                tr("launch_ready", lang),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(tr("launch_conn_desc", lang)),
            Line::from(""),
            Line::from(vec![
                Span::raw(format!("{} ", tr("launch_stat", lang))),
                if actual_running {
                    Span::styled(
                        if is_ru {
                            "ОБНАРУЖЕНО (ГОТОВО К СТАРТУ)"
                        } else {
                            "DETECTED (READY TO START)"
                        },
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        if is_ru {
                            "ОЖИДАНИЕ SIMULATOR..."
                        } else {
                            "WAITING FOR SIMULATOR..."
                        },
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    )
                },
            ]),
        ],
        1 => vec![
            Line::from(Span::styled(
                tr("launch_conf_title", lang),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(tr("launch_conf_desc", lang)),
        ],
        2 => vec![
            Line::from(Span::styled(
                tr("launch_lang_title", lang),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(tr("launch_lang_desc", lang)),
        ],
        3 => vec![
            Line::from(Span::styled(
                tr("launch_doc_title", lang),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                tr("launch_nav", lang),
                Style::default().add_modifier(Modifier::UNDERLINED),
            )),
            Line::from(tr("launch_nav_desc", lang)),
            Line::from(""),
            Line::from(Span::styled(
                tr("launch_feat", lang),
                Style::default().add_modifier(Modifier::UNDERLINED),
            )),
            Line::from(tr("launch_feat_desc", lang)),
        ],
        4 => vec![
            Line::from(Span::styled(
                tr("launch_cred_title", lang),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("AC Pro Engineer Tool"),
            Line::from(format!("Version: {}", ac_core::updater::CURRENT_VERSION)),
            Line::from(""),
            Line::from(Span::styled(
                tr("launch_created", lang),
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  ***SH:)",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(tr("launch_thanks", lang)),
            Line::from("  Kunos Simulazioni (Assetto Corsa)"),
            Line::from("  Rust Community (Ratatui, Serde, Tauri)"),
            Line::from(""),
            Line::from("© 2026 All Rights Reserved."),
        ],
        5 => {
            let mut lines = vec![];
            if let UpdateStatus::Downloading(pct) = *update_status {
                lines.push(Line::from(Span::styled(
                    if is_ru {
                        "Загрузка..."
                    } else {
                        "Downloading..."
                    },
                    Style::default().fg(Color::Cyan),
                )));
                // The bar is 20 cells wide, so `filled` has to be capped
                // whatever the percentage says: `20 - filled` is an unsigned
                // subtraction and would panic on anything over 100%.
                const BAR_CELLS: usize = 20;
                let filled = ((pct / 5.0) as usize).min(BAR_CELLS);
                let bar = "█".repeat(filled) + &"░".repeat(BAR_CELLS - filled);
                lines.push(Line::from(Span::styled(
                    format!("{} {:.1}%", bar, pct),
                    Style::default().fg(Color::Cyan),
                )));
            } else if let UpdateStatus::Downloaded(_) = *update_status {
                lines.push(Line::from(Span::styled(
                    if is_ru { "ГОТОВО!" } else { "READY!" },
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(if is_ru {
                    "Нажмите ENTER..."
                } else {
                    "Press ENTER..."
                }));
            } else if let Some(info) = app.updater.get_selected_release() {
                lines.push(Line::from(vec![
                    Span::raw("ver: "),
                    Span::styled(
                        " < ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" v{} ", info.version),
                        Style::default()
                            .fg(if info.is_latest {
                                Color::LightGreen
                            } else {
                                Color::White
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " > ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    if is_ru {
                        " [←/→] Выбор версии   [ENTER] Установка"
                    } else {
                        " [←/→] Select Version   [ENTER] Install"
                    },
                    Style::default().fg(Color::DarkGray).bg(Color::Black),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    if is_ru {
                        "Список изменений:"
                    } else {
                        "Changelog:"
                    },
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(Span::styled(
                    info.notes.clone(),
                    Style::default().fg(Color::Gray),
                )));
                if is_legacy_version(&info.version) {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        if is_ru {
                            "⚠️ ВНИМАНИЕ: Старая версия!"
                        } else {
                            "⚠️ WARNING: Legacy Version!"
                        },
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(Span::styled(
                        if is_ru {
                            "В ней нет апдейтера. Вы не сможете вернуться обратно."
                        } else {
                            "No updater inside. You won't be able to switch back."
                        },
                        Style::default().fg(Color::Red),
                    )));
                }
            } else {
                if let UpdateStatus::Checking = *update_status {
                    lines.push(Line::from("Checking GitHub..."));
                } else {
                    lines.push(Line::from(Span::styled(
                        "No releases found.",
                        Style::default().fg(Color::Red),
                    )));
                }
            }
            lines
        }
        6 => vec![
            Line::from(Span::styled(
                tr("launch_shut_title", lang),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
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

fn render_status_bar(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;
    let update_status = app.updater.status.lock().unwrap_or_else(|e| e.into_inner());

    let (msg, color) = match *update_status {
        UpdateStatus::UpdateAvailable => (
            if is_ru {
                "🔥 ДОСТУПНО ОБНОВЛЕНИЕ"
            } else {
                "🔥 UPDATE AVAILABLE"
            }
            .to_string(),
            Color::LightGreen,
        ),
        UpdateStatus::Downloading(_) => (
            if is_ru {
                "♻ Скачивание..."
            } else {
                "♻ Downloading..."
            }
            .to_string(),
            Color::Cyan,
        ),
        _ => {
            let actual_running = app.is_game_running;
            if actual_running {
                (tr("launch_on", lang), Color::Green)
            } else {
                (
                    if is_ru {
                        "ОЖИДАНИЕ СИМУЛЯТОРА..."
                    } else {
                        "WAITING FOR SIMULATOR..."
                    }
                    .to_string(),
                    Color::Yellow,
                )
            }
        }
    };

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
        ])
        .split(area);

    let status = Paragraph::new(msg).style(Style::default().fg(color).add_modifier(Modifier::BOLD));
    // The launcher's keys, all of them. This named two of six: ←/→ changes the
    // language and the release on the update row, O opens the review page, H
    // hides the banner and Q leaves, and none of that was written anywhere on
    // the screen it works on.
    let controls_hint = if is_ru {
        "[↑/↓] Навигация  [←/→] Менять  [ENTER] Выбор  [Q] Выход"
    } else {
        "[↑/↓] Select  [←/→] Change  [ENTER] Open  [Q] Quit"
    };
    let controls = Paragraph::new(controls_hint)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    let copyright = Paragraph::new(format!("v{}", ac_core::updater::CURRENT_VERSION))
        .alignment(Alignment::Right)
        .style(Style::default().fg(Color::DarkGray));

    f.render_widget(status, layout[0]);
    f.render_widget(controls, layout[1]);
    f.render_widget(copyright, layout[2]);

    let border = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
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

fn is_legacy_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() < 3 {
        return false;
    }
    let parse_part = |s: &str| -> u32 {
        s.chars()
            .take_while(|c| c.is_numeric())
            .collect::<String>()
            .parse()
            .unwrap_or(0)
    };
    let major = parse_part(parts[0]);
    let minor = parse_part(parts[1]);
    let patch = parse_part(parts[2]);

    if major > 0 {
        return false;
    }
    if minor > 1 {
        return false;
    }
    if minor == 1 {
        return patch < 4;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppStage;
    use ac_core::overlay::bridge::{BridgeInfo, BridgeStatus, Complaint};
    use ac_core::overlay::frame::{OVERLAY_MMF_NAME, OverlayFrame};
    use ratatui::backend::TestBackend;

    /// Draw the launcher with the overlay card up and read the cells back.
    ///
    /// The card is the only place most people will ever see a version
    /// mismatch, and until this existed the only way to know it drew the
    /// sentence rather than the first 64 columns of it was to run the
    /// application and look.
    fn card_text(status: BridgeStatus, width: u16, height: u16) -> String {
        let mut app = AppState::new(ac_core::overlay::OverlayMode::External);
        app.stage = AppStage::Launcher;
        app.onboarding = OverlayOnboarding::Done;
        app.show_first_run_prompt = false;
        app.show_overlay_card = true;
        app.bridge_status = status;

        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        terminal
            .draw(|f| render(f, f.size(), &app))
            .expect("the card draws");

        let buffer = terminal.backend().buffer().clone();
        let mut rows: Vec<String> = Vec::new();
        for y in 0..height {
            let mut row = String::new();
            for x in 0..width {
                row.push_str(buffer.get(x, y).symbol());
            }
            rows.push(row);
        }

        // Only what is inside the card's own borders. The launcher draws a
        // menu to its left, so joining whole rows would splice unrelated text
        // into the middle of a wrapped sentence.
        let mut card = String::new();
        for row in &rows {
            let cells: Vec<usize> = row
                .char_indices()
                .filter(|(_, c)| *c == '\u{2551}')
                .map(|(i, _)| i)
                .collect();
            if let (Some(first), Some(last)) = (cells.first(), cells.last())
                && last > first
            {
                card.push_str(row[first + '\u{2551}'.len_utf8()..*last].trim_end());
                card.push('\n');
            }
        }

        // Wrapping means a sentence can arrive over two rows, and a test for
        // "the whole thing is on screen" should not care which. Row breaks
        // become spaces; runs of spaces collapse.
        card.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn stale_bridge() -> BridgeStatus {
        BridgeStatus::Incompatible {
            info: Box::new(BridgeInfo {
                protocol: 1,
                version: "0.2.9".to_string(),
                frame_bytes: 376,
                mmf: OVERLAY_MMF_NAME.to_string(),
                pid: 32,
            }),
            complaint: Complaint::FrameBytes {
                found: 376,
                expected: size_of::<OverlayFrame>(),
            },
        }
    }

    /// The whole sentence, not the half that fitted. A fixed 66-column card
    /// cut this diagnostic before it reached the part naming the remedy.
    #[test]
    fn a_bridge_complaint_is_drawn_whole() {
        let screen = card_text(stale_bridge(), 140, 40);
        assert!(screen.contains("376"), "the size it maps:\n{screen}");
        assert!(
            screen.contains("CSP will not open it"),
            "the consequence, which is past column 66:\n{screen}"
        );
        assert!(
            screen.contains("press B"),
            "and the remedy, which is past that:\n{screen}"
        );
        assert!(
            // Derived, not written out: the frame's size is a thing that
            // changes, and a test that hardcodes it fails for the wrong reason
            // the next time a field is added.
            screen.contains(&format!(
                "maps 376 bytes, this build's frame is {}",
                size_of::<OverlayFrame>()
            )),
            "the two numbers belong in one sentence:\n{screen}"
        );
    }

    /// A bridge running but too old to announce itself needs the opposite
    /// advice to one that is not running, and getting them the wrong way round
    /// sends the driver to start the bridge that is already the problem.
    #[test]
    fn an_unannounced_bridge_is_not_reported_as_absent() {
        let screen = card_text(BridgeStatus::Unannounced, 140, 40);
        assert!(
            screen.contains("too old"),
            "an unannounced bridge is old, not missing:\n{screen}"
        );
        assert!(
            !screen.contains("not running"),
            "it is running, and saying otherwise is the wrong remedy:\n{screen}"
        );
    }

    /// Windows has no bridge, so the card must not offer to fetch one.
    #[test]
    fn windows_is_not_offered_a_bridge_to_fetch() {
        let screen = card_text(BridgeStatus::NotRequired, 140, 40);
        assert!(screen.contains("not needed"), "{screen}");
        assert!(
            !screen.contains("B — fetch"),
            "there is nothing to fetch on Windows:\n{screen}"
        );
    }

    /// The card is sized from its content, so it has to survive a terminal
    /// smaller than the content it wants to draw rather than panicking.
    #[test]
    fn the_card_survives_a_terminal_too_small_for_it() {
        for (width, height) in [(20u16, 6u16), (40, 10), (80, 24)] {
            let _ = card_text(stale_bridge(), width, height);
        }
    }
}
