//! Sharing this session, and watching somebody else's.
//!
//! **Two keys and a list.** Everything on this screen is one of three things:
//! what the network is doing right now, who else is on it, and the settings
//! somebody might want after they have it working. In that order, because that
//! is the order they are needed in — a screen that opens on a form asking for
//! a port is a screen nobody uses twice.
//!
//! The keys come from [`crate::keys`] like every other key in this program, so
//! the hints at the bottom cannot name one that does nothing. Nothing here
//! opens a socket: it moves [`ac_core::lan::LanWish`], and the tick reconciles
//! it — see `AppState::reconcile_lan`.

use crate::AppState;
use ac_core::broadcast::discovery::Role;
use ac_core::i18n::Translate;
use ac_core::lan::Mode;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", "SHARING AND WATCHING".tr_lang(lang)))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            // Both switches and what each is doing.
            Constraint::Length(7),
            // Everybody on the network — the rest of the screen, because it is
            // the part that grows.
            Constraint::Min(5),
            // The settings that are not a switch.
            Constraint::Length(4),
        ])
        .split(inner);

    render_switches(f, rows[0], app);
    render_peers(f, rows[1], app);
    render_deeper(f, rows[2], app);
}

/// What the two switches are doing, and what is wrong if anything is.
fn render_switches(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let keys = &app.config.keys;
    let wish = &app.lan;
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let on = Style::default()
        .fg(Color::Black)
        .bg(Color::Green)
        .add_modifier(Modifier::BOLD);
    let off = Style::default().fg(Color::DarkGray);
    let label = Style::default().fg(Color::Gray);
    let value = Style::default().fg(Color::White);

    // --- sharing ---------------------------------------------------------
    let sharing = wish.mode.sends();
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!(" {} ", crate::keys::describe(&keys.lan_share)),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                if sharing {
                    format!(" {} ", "SENDING".tr_lang(lang))
                } else {
                    format!(" {} ", "not sending".tr_lang(lang))
                },
                if sharing { on } else { off },
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{:<14}", "you are".tr_lang(lang)), label),
            Span::styled(
                if wish.share_as.trim().is_empty() {
                    "somebody".tr_lang(lang).to_string()
                } else {
                    wish.share_as.clone()
                },
                value,
            ),
        ]),
        Line::from(vec![
            Span::styled(format!("{:<14}", "sending to".tr_lang(lang)), label),
            match wish.share_to.trim().is_empty() {
                // **The one thing a person still has to choose**, said where
                // they are looking rather than in a manual.
                true => Span::styled(
                    "nobody yet — choose below".tr_lang(lang),
                    Style::default().fg(Color::Yellow),
                ),
                false => Span::styled(wish.share_to.clone(), value),
            },
        ]),
    ];
    if sharing {
        let sent = app.sharing.as_ref().map(|link| link.sent()).unwrap_or(0);
        lines.push(Line::from(vec![
            Span::styled(format!("{:<14}", "sent".tr_lang(lang)), label),
            Span::styled(
                format!("{sent} · {:.0}/s", wish.share_hz),
                Style::default().fg(Color::Cyan),
            ),
        ]));
    }
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::RIGHT)
                .title(format!(" {} ", "SHARE".tr_lang(lang))),
        ),
        halves[0],
    );

    // --- watching --------------------------------------------------------
    let watching = wish.mode.receives();
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!(" {} ", crate::keys::describe(&keys.lan_watch)),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                if watching {
                    format!(" {} ", "WATCHING".tr_lang(lang))
                } else {
                    format!(" {} ", "not watching".tr_lang(lang))
                },
                if watching { on } else { off },
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{:<14}", "listening on".tr_lang(lang)), label),
            Span::styled(
                match app.link.listening_on.is_empty() {
                    true => wish.listen_on.clone(),
                    false => app.link.listening_on.clone(),
                },
                value,
            ),
        ]),
    ];
    lines.push(match (&app.link.from, app.link.quiet) {
        (Some(who), false) => Line::from(vec![
            Span::styled(format!("{:<14}", "from".tr_lang(lang)), label),
            Span::styled(who.clone(), Style::default().fg(Color::Green)),
            Span::styled(
                format!(
                    "  {:.0}/s  {} ms",
                    app.link.rate_hz, app.link.age_ms
                ),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        // **Heard from and then not** is a different state from never having
        // heard anybody, and a watcher staring at a frozen screen needs to be
        // told which one this is.
        (Some(who), true) => Line::from(vec![
            Span::styled(format!("{:<14}", "from".tr_lang(lang)), label),
            Span::styled(
                format!("{who} — {}", "gone quiet".tr_lang(lang)),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        (None, _) if watching => Line::from(vec![
            Span::styled(format!("{:<14}", "from".tr_lang(lang)), label),
            Span::styled(
                "nobody is sending here yet".tr_lang(lang),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        (None, _) => Line::from(""),
    });
    if app.link.lost > 0 {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<14}", "lost".tr_lang(lang)), label),
            Span::styled(
                format!("{}", app.link.lost),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default().title(format!(" {} ", "WATCH".tr_lang(lang))),
        ),
        halves[1],
    );
}

/// Everybody running this program on this network.
fn render_peers(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let keys = &app.config.keys;
    let block = Block::default()
        .borders(Borders::TOP)
        .title(format!(
            " {}   [{}] {}   [↑↓] {} ",
            "ON THIS NETWORK".tr_lang(lang),
            crate::keys::describe(&keys.lan_pick),
            "send to them".tr_lang(lang),
            "choose".tr_lang(lang),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.peers.is_empty() {
        // **Why the list is empty matters more than that it is.** A group that
        // could not be joined and a network with nobody on it look the same,
        // and only one of them is worth doing something about.
        let (text, colour) = match (&app.lan_trouble, app.finder.is_some()) {
            (Some(why), _) => (why.clone(), Color::Yellow),
            (None, true) => (
                "nobody else yet — start the program on the other machine"
                    .tr_lang(lang)
                    .to_string(),
                Color::DarkGray,
            ),
            (None, false) => (
                "press a key below to switch the network on"
                    .tr_lang(lang)
                    .to_string(),
                Color::DarkGray,
            ),
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                text,
                Style::default().fg(colour),
            ))),
            inner,
        );
        return;
    }

    let rows: Vec<Row<'_>> = app
        .peers
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            let chosen = index == app.peer_cursor;
            let aimed_at = app.lan.share_to.trim() == peer.address();
            let mark = match (chosen, aimed_at) {
                (true, true) => "▸●",
                (true, false) => "▸ ",
                (false, true) => " ●",
                (false, false) => "  ",
            };
            let role = match peer.role {
                Role::Driving => Span::styled(
                    peer.role.label().tr_lang(lang),
                    Style::default().fg(Color::Green),
                ),
                _ => Span::styled(
                    peer.role.label().tr_lang(lang),
                    Style::default().fg(Color::DarkGray),
                ),
            };
            let doing = match (peer.car.is_empty(), peer.track.is_empty()) {
                (false, false) => format!("{} · {}", peer.car, peer.track),
                (false, true) => peer.car.clone(),
                _ => String::new(),
            };
            Row::new(vec![
                Cell::from(mark),
                Cell::from(peer.name.clone()).style(if chosen {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                }),
                Cell::from(match peer.reachable() {
                    true => peer.address(),
                    // Somebody who is not listening cannot be sent to, and
                    // offering it would build a link that carries nothing.
                    false => "not listening".tr_lang(lang).to_string(),
                }),
                Cell::from(role),
                Cell::from(doing),
                Cell::from(format!("{} {}", peer.front_end, peer.version))
                    .style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(18),
            Constraint::Length(22),
            Constraint::Length(10),
            Constraint::Min(16),
            Constraint::Length(18),
        ],
    );
    f.render_widget(table, inner);
}

/// The settings that are not one of the two switches.
fn render_deeper(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let keys = &app.config.keys;
    let wish = &app.lan;
    let label = Style::default().fg(Color::Gray);
    let value = Style::default().fg(Color::White);
    let yes_no = |on: bool| if on { "yes" } else { "no" }.tr_lang(lang);

    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("[{}] ", crate::keys::describe(&keys.lan_announce)),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(format!("{} ", "findable on the network".tr_lang(lang)), label),
        Span::styled(yes_no(wish.announce), value),
        Span::styled("   ", label),
        Span::styled(format!("{} ", "rate".tr_lang(lang)), label),
        Span::styled(format!("{:.0}/s", wish.share_hz), value),
        Span::styled("   ", label),
        Span::styled(
            format!("{} ", "only while on track".tr_lang(lang)),
            label,
        ),
        Span::styled(yes_no(wish.only_on_track), value),
    ])];

    // **What is stopping it, said out loud.** Sending switched on with nowhere
    // to send it is the one mistake somebody makes at the moment it matters,
    // and a switch that says SENDING cannot show it on its own.
    if let Some(complaint) = wish.complaint() {
        lines.push(Line::from(Span::styled(
            complaint.tr_lang(lang),
            Style::default().fg(Color::Yellow),
        )));
    } else if wish.mode == Mode::Off {
        lines.push(Line::from(Span::styled(
            "nothing leaves this machine".tr_lang(lang),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(Span::styled(
        format!(
            "{}  ·  {}",
            "everything else is in Settings → SHARING".tr_lang(lang),
            "the same file both programs read".tr_lang(lang),
        ),
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP)),
        area,
    );
}
