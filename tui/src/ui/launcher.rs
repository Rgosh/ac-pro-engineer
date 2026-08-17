use crate::{AppState, OverlayOnboarding};
use ac_core::config::Language;
use ac_core::i18n::{Translate, tr_fmt};
use ac_core::updater::UpdateStatus;
use ratatui::{prelude::*, widgets::*};
use std::sync::atomic::AtomicBool;

pub static SHOW_REVIEW_BANNER: AtomicBool = AtomicBool::new(true);

/// The launcher's rows, in the order they are drawn.
///
/// Named because they were numbers, in six places across two files: the menu
/// itself, the two `match`es behind the information panel, the arrow keys, the
/// Enter handler and the update recheck. Inserting a row meant renumbering all
/// of them and being right every time, and being wrong meant Enter opening the
/// documentation from the credits — which is not a compile error anywhere.
pub const ROW_START: usize = 0;
pub const ROW_SETTINGS: usize = 1;
/// Which simulator this program is working with. See [`ROW_LAST`] for why the
/// list is addressed this way at all.
pub const ROW_GAME: usize = 2;
pub const ROW_LANGUAGE: usize = 3;
pub const ROW_DOCUMENTATION: usize = 4;
pub const ROW_CREDITS: usize = 5;
pub const ROW_UPDATES: usize = 6;
pub const ROW_EXIT: usize = 7;
/// The last row the selection can reach.
pub const ROW_LAST: usize = ROW_EXIT;

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
        // Not "set the path in Settings": that screen has no such row and
        // never has had one, so the message sent people to a dead end. The
        // only way to point this at an install it cannot find is
        // `ac_install_path` in the config file.
        None => lines.push(Line::from(Span::styled(
            "No Assetto Corsa found — set ac_install_path in config.json.",
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
            Span::styled("not found — set ac_install_path in config.json", bad),
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

    // Why it is not installed, when the answer is known.
    //
    // The application writes the panel by itself and says whether the files
    // are there — and when writing them failed, said only that they were not,
    // which reads as "it has not got round to it yet" rather than as an error.
    // A game folder that cannot be written to is the likeliest cause and the
    // one nobody guesses, so the operating system's own words go on the card.
    if !report.current && app.overlay_install_status.starts_with("could not install") {
        lines.push(Line::from(vec![
            Span::styled("         ", dim),
            Span::styled(app.overlay_install_status.clone(), bad),
        ]));
    }

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
        .title(" UPDATE ".tr(is_ru))
        .title_alignment(Alignment::Center);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "SUCCESSFULLY UPDATED!".tr(is_ru),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("v{}", ac_core::updater::CURRENT_VERSION)),
        Line::from(""),
        Line::from(Span::styled(
            "Press ENTER to continue".tr(is_ru),
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
    let text = "⭐ This is an Open Source project. Your review helps us grow!".tr(is_ru);
    let hint = "[O] Leave Review  [H] Hide Forever".tr(is_ru);

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
        UpdateStatus::Downloading(pct) => format!("♻   {}: {:.0}%", "Downloading".tr(is_ru), pct),
        UpdateStatus::UpdateAvailable => format!("🔥  {}!", "AVAILABLE".tr(is_ru)),
        UpdateStatus::Checking => format!("⏳  {}", "Checking...".tr(is_ru)),
        UpdateStatus::NoUpdate => format!("✅  {}", "Versions & Rollback".tr(is_ru)),
        UpdateStatus::Error(_) => format!("❌  {}", "Net Error".tr(is_ru)),
        _ => format!("♻   {}", "CHECK UPDATES".tr_lang(lang)),
    };

    let menu_items = [
        format!("🖥️  {}", "START (TERMINAL TUI)".tr(is_ru)),
        format!("⚙️   {}", "SETTINGS".tr_lang(lang)),
        // The game's own name, untranslated: it is what it calls itself. The
        // short one, because this column is 36 cells wide — the panel beside
        // it has the room to say it in full.
        format!("🏁  {}: < {} >", "GAME".tr_lang(lang), app.game.short_name),
        match app.config.language {
            Language::English => "LANGUAGE: < ENGLISH >",
            Language::Russian => "ЯЗЫК: < РУССКИЙ >",
        }
        .to_string(),
        format!("📚  {}", "DOCUMENTATION".tr_lang(lang)),
        format!("👤  {}", "CREDITS / AUTHOR".tr_lang(lang)),
        update_label,
        format!("❌  {}", "EXIT".tr_lang(lang)),
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

            if i == ROW_UPDATES
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
            .title(" MAIN MENU ".tr_lang(lang).to_string())
            .title_alignment(Alignment::Center),
    );

    f.render_widget(list, area);
}

/// What choosing a game means, said before it is chosen rather than after.
///
/// Three things a driver has to be able to see here, because none of them is
/// discoverable from anywhere else:
///
/// * **whether the game is up.** The row above is a choice, not a detection,
///   so this is where detection still gets to speak.
/// * **what this game can and cannot measure.** Competizione publishes no tyre
///   wear and no tread temperatures, so the wear and camber advice go quiet —
///   and advice going quiet with no explanation reads exactly like a broken
///   feature.
/// * **that setups may not exist here.** One game keeps them where this
///   program can read them and the other does not.
fn game_panel(app: &AppState) -> Vec<Line<'static>> {
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;
    let dim = Style::default().fg(Color::DarkGray);

    let mut lines = vec![
        Line::from(Span::styled(
            app.game.name.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(format!("{} ", "Connection Status:".tr_lang(lang))),
            if app.is_game_running {
                Span::styled(
                    "DETECTED (READY TO START)".tr(is_ru),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    "WAITING FOR SIMULATOR...".tr(is_ru),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::ITALIC),
                )
            },
        ]),
        Line::from(""),
    ];

    // What the advice will and will not be able to say, from the game's own
    // capabilities rather than from a list written out by hand here — the
    // whole point of the flags is that one place knows.
    if let Some(backend) = app.game.backend() {
        let measured = |yes: bool, what: &str| {
            Line::from(vec![
                Span::styled(
                    if yes { "  ✔  " } else { "  —  " },
                    Style::default().fg(if yes { Color::Green } else { Color::DarkGray }),
                ),
                Span::styled(
                    what.tr_lang(lang).to_string(),
                    if yes {
                        Style::default().fg(Color::Gray)
                    } else {
                        dim
                    },
                ),
            ])
        };
        lines.push(Line::from(Span::styled(
            "This game reports:".tr_lang(lang).to_string(),
            Style::default().add_modifier(Modifier::UNDERLINED),
        )));
        lines.push(measured(backend.capabilities.tyre_wear, "Tyre wear"));
        lines.push(measured(
            backend.capabilities.tyre_edge_temps,
            "Tread temperatures (camber advice)",
        ));
        // Where one game is blind the other is not, and this is the pair that
        // shows it: Competizione trades tyre wear for brake wear.
        lines.push(measured(backend.capabilities.brake_wear, "Brake pad wear"));
        lines.push(measured(backend.capabilities.track_grip, "Track grip"));
        lines.push(measured(backend.capabilities.lap_validity, "Track limits"));
        lines.push(measured(backend.capabilities.sectors, "Sector times"));
        lines.push(measured(backend.capabilities.setups, "Setups on disk"));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Use LEFT / RIGHT arrows to choose the simulator."
            .tr_lang(lang)
            .to_string(),
        dim,
    )));
    lines
}

fn render_info_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;
    let update_status = app.updater.status.lock().unwrap_or_else(|e| e.into_inner());

    let title = match app.launcher_selection {
        ROW_START => " INFORMATION ".tr_lang(lang).to_string(),
        ROW_SETTINGS => "APP CONFIGURATION".tr_lang(lang).to_string(),
        ROW_GAME => "SIMULATOR".tr_lang(lang).to_string(),
        ROW_LANGUAGE => "INTERFACE LANGUAGE".tr_lang(lang).to_string(),
        ROW_DOCUMENTATION => "USER MANUAL".tr_lang(lang).to_string(),
        ROW_CREDITS => "CREDITS & AUTHOR".tr_lang(lang).to_string(),
        ROW_UPDATES => "SYSTEM UPDATE".tr_lang(lang).to_string(),
        ROW_EXIT => "SHUTDOWN".tr_lang(lang).to_string(),
        _ => " INFORMATION ".tr_lang(lang).to_string(),
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
        ROW_START => vec![
            Line::from(Span::styled(
                "TERMINAL MODE (TUI)",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "READY TO RACE".tr_lang(lang).to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(tr_fmt(
                "Reads {0}'s shared memory. Make sure the game is running.",
                is_ru,
                &[app.game.name],
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw(format!("{} ", "Connection Status:".tr_lang(lang))),
                if actual_running {
                    Span::styled(
                        "DETECTED (READY TO START)".tr(is_ru),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        "WAITING FOR SIMULATOR...".tr(is_ru),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    )
                },
            ]),
        ],
        ROW_SETTINGS => vec![
            Line::from(Span::styled(
                "APP CONFIGURATION".tr_lang(lang).to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Press ENTER to open settings.".tr_lang(lang).to_string()),
        ],
        ROW_GAME => game_panel(app),
        ROW_LANGUAGE => vec![
            Line::from(Span::styled(
                "INTERFACE LANGUAGE".tr_lang(lang).to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(
                "Use LEFT / RIGHT arrows to switch language instantly."
                    .tr_lang(lang)
                    .to_string(),
            ),
        ],
        ROW_DOCUMENTATION => vec![
            Line::from(Span::styled(
                "USER MANUAL".tr_lang(lang).to_string(),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Navigation:".tr_lang(lang).to_string(),
                Style::default().add_modifier(Modifier::UNDERLINED),
            )),
            Line::from(
                " F1-F8 : Switch Tabs
 Q     : Return / Quit
 Arrows: Navigate"
                    .tr_lang(lang)
                    .to_string(),
            ),
            Line::from(""),
            Line::from(Span::styled(
                "Features:".tr_lang(lang).to_string(),
                Style::default().add_modifier(Modifier::UNDERLINED),
            )),
            Line::from(
                " • Telemetry: Live graphs
 • Engineer: Real-time advice
 • Analysis: Lap comparison"
                    .tr_lang(lang)
                    .to_string(),
            ),
        ],
        ROW_CREDITS => vec![
            Line::from(Span::styled(
                "CREDITS & AUTHOR".tr_lang(lang).to_string(),
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("AC Pro Engineer Tool"),
            Line::from(format!("Version: {}", ac_core::updater::CURRENT_VERSION)),
            Line::from(""),
            Line::from(Span::styled(
                "Created by:".tr_lang(lang).to_string(),
                Style::default().fg(Color::Gray),
            )),
            Line::from(Span::styled(
                "  ***SH:)",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Special thanks to:".tr_lang(lang).to_string()),
            Line::from("  Kunos Simulazioni (Assetto Corsa)"),
            Line::from("  Rust Community (Ratatui, Serde, Tauri)"),
            Line::from(""),
            Line::from("© 2026 All Rights Reserved."),
        ],
        ROW_UPDATES => {
            let mut lines = vec![];
            if let UpdateStatus::Downloading(pct) = *update_status {
                lines.push(Line::from(Span::styled(
                    "Downloading...".tr(is_ru),
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
                    "READY!".tr(is_ru),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from("Press ENTER...".tr(is_ru)));
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
                    " [←/→] Select Version   [ENTER] Install".tr(is_ru),
                    Style::default().fg(Color::DarkGray).bg(Color::Black),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Changelog:".tr(is_ru),
                    Style::default().fg(Color::Cyan),
                )));
                lines.push(Line::from(Span::styled(
                    info.notes.clone(),
                    Style::default().fg(Color::Gray),
                )));
                if is_legacy_version(&info.version) {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "⚠️ WARNING: Legacy Version!".tr(is_ru),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(Span::styled(
                        "No updater inside. You won't be able to switch back.".tr(is_ru),
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
        ROW_EXIT => vec![
            Line::from(Span::styled(
                "SHUTDOWN".tr_lang(lang).to_string(),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(
                "Data is saved automatically.
Press ENTER to close."
                    .tr_lang(lang)
                    .to_string(),
            ),
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
            "🔥 UPDATE AVAILABLE".tr(is_ru).to_string(),
            Color::LightGreen,
        ),
        UpdateStatus::Downloading(_) => ("♻ Downloading...".tr(is_ru).to_string(), Color::Cyan),
        _ => {
            let actual_running = app.is_game_running;
            if actual_running {
                ("✓ System Online".tr_lang(lang).to_string(), Color::Green)
            } else {
                (
                    "WAITING FOR SIMULATOR...|spelled out".tr(is_ru).to_string(),
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
    let controls_hint = "[↑/↓] Select  [←/→] Change  [ENTER] Open  [Q] Quit".tr(is_ru);
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
        let mut app = AppState::new();
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
