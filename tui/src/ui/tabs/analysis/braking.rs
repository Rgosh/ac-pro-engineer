//! What the brake pedal did wrong, and what it cost.
//!
//! **Half a lap time lives in braking** and nothing in this program looked at
//! it. CORNERS says which corner cost the time; this says what was done with
//! the pedal in it and what to do instead.
//!
//! It decides nothing. `ac_core::braking::look` reads the same decomposition
//! the CORNERS sub-tab draws and names which of the five things a driver can
//! do wrong with a brake pedal they are doing — soft, early, released,
//! trailed, or slow through the middle on braking that matched. The rule about
//! one fault per corner, the thresholds under which a difference is the
//! measurement rather than the driver, and the order the findings are read in
//! are all written down beside the numbers in `core/src/braking.rs`.
//!
//! The findings come first and the table second, for the same reason CORNERS
//! filters: a driver who reads a hundred numbers leaves knowing T7 was slow.
//! "You reach 1.50 g where the reference reaches 1.77, in three corners, and
//! it costs you 0.42 s" is a thing to go and practise.

use crate::AppState;
use ac_core::analyzer::LapData;
use ac_core::braking::{AtCorner, Report};
use ac_core::i18n::{Translate, tr_fmt};
use ratatui::{prelude::*, widgets::*};

pub fn render(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap: &LapData,
    reference: Option<&LapData>,
) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;
    let border = Style::default().fg(app.ui_state.get_color(&theme.border));

    let Some(reference) = reference else {
        message(
            f,
            area,
            border,
            "Every number here is a difference from a reference lap. Drive a second one, or \
             load a saved lap with 'L'."
                .tr(is_ru),
        );
        return;
    };

    // The same lap against itself is every delta at zero, which reads as
    // perfect braking rather than as the tautology it is.
    if std::ptr::eq(lap, reference) {
        message(
            f,
            area,
            border,
            "This is the reference lap — there is nothing to compare it with.".tr(is_ru),
        );
        return;
    }

    let report = app
        .ui_state
        .analysis
        .corner_cache
        .borrow_mut()
        .braking(lap, reference);

    if report.corners.is_empty() {
        message(
            f,
            area,
            border,
            "No braking zones found — braking is detected from the pedal and the lateral load, \
             not from a track map."
                .tr(is_ru),
        );
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(9),
            Constraint::Min(6),
        ])
        .split(area);

    render_header(f, layout[0], app, &report, is_ru);
    render_advice(f, layout[1], app, &report, is_ru);
    render_zones(f, layout[2], app, &report, is_ru);
}

fn message(f: &mut Frame<'_>, area: Rect, border: Style, text: &str) {
    let block = Block::default().borders(Borders::ALL).border_style(border);
    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(block),
        area,
    );
}

/// The one number: what braking cost, and across how many corners.
fn render_header(f: &mut Frame<'_>, area: Rect, app: &AppState, report: &Report, is_ru: bool) {
    let theme = &app.ui_state.theme;
    let cost = report.cost();
    let faults = report
        .corners
        .iter()
        .filter(|corner| corner.fault.is_some())
        .count();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)))
        .title(" BRAKING ".tr(is_ru).to_string());

    if faults == 0 {
        f.render_widget(
            Paragraph::new(
                "The brakes cost nothing this lap against the reference."
                    .tr(is_ru)
                    .to_string(),
            )
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Center)
            .block(block),
            area,
        );
        return;
    }

    let line = Line::from(vec![
        Span::styled(
            format!(" {:.2} s ", cost as f32 / 1000.0),
            Style::default()
                .fg(if cost >= ac_core::braking::SERIOUS_MS {
                    Color::Red
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            tr_fmt(
                "lost between entry and apex, across {0} corners",
                is_ru,
                &[&faults.to_string()],
            ),
            Style::default().fg(app.ui_state.get_color(&theme.text)),
        ),
    ]);

    f.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

/// What to work on: the core's findings, worst first.
///
/// The whole case and not the headline — the mechanism, what it did, and what
/// to look at next run — because a message and an action alone is the
/// difference between a warning light and an engineer.
fn render_advice(f: &mut Frame<'_>, area: Rect, app: &AppState, report: &Report, is_ru: bool) {
    let theme = &app.ui_state.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)))
        .title(" WHAT TO WORK ON ".tr(is_ru).to_string());

    let advice = report.advice();
    if advice.is_empty() {
        f.render_widget(
            Paragraph::new(
                "Nothing in the braking is costing enough to be worth an evening — the pedal \
                 matches the reference inside what the telemetry can resolve."
                    .tr(is_ru)
                    .to_string(),
            )
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(block),
            area,
        );
        return;
    }

    let mut lines: Vec<Line<'_>> = Vec::new();
    for one in &advice {
        let colour = match one.severity {
            ac_core::engineer::Severity::Critical => Color::Red,
            ac_core::engineer::Severity::Warning => Color::Yellow,
            _ => Color::Cyan,
        };
        lines.push(Line::from(Span::styled(
            one.message.clone(),
            Style::default().fg(colour).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::styled("  » ", Style::default().fg(colour)),
            Span::styled(
                one.action.clone(),
                Style::default().fg(app.ui_state.get_color(&theme.text)),
            ),
        ]));
        if let Some(chain) = &one.chain {
            lines.push(Line::from(Span::styled(
                format!("     why: {}", chain.cause),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                format!("     check next run: {}", chain.confirm),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}

/// Every zone, one row each.
fn render_zones(f: &mut Frame<'_>, area: Rect, app: &AppState, report: &Report, is_ru: bool) {
    let theme = &app.ui_state.theme;
    let text = app.ui_state.get_color(&theme.text);

    let rows: Vec<Row<'_>> = report.corners.iter().map(|zone| row(zone, text)).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Min(9),
        ],
    )
    .header(
        Row::new(vec![
            "CORNER".tr(is_ru),
            "LOST".tr(is_ru),
            "BRAKE PT".tr(is_ru),
            "PEAK".tr(is_ru),
            "vs REF".tr(is_ru),
            "TRAIL".tr(is_ru),
            "MIN SPD".tr(is_ru),
            "VERDICT".tr(is_ru),
        ])
        .style(
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)))
            .title(
                " EVERY BRAKING ZONE, AGAINST THE REFERENCE "
                    .tr(is_ru)
                    .to_string(),
            ),
    );

    f.render_widget(table, area);
}

fn row(zone: &AtCorner, text: Color) -> Row<'static> {
    let colour = match zone.fault {
        Some(_) => Color::Yellow,
        None => text,
    };

    // A blank is no answer: one of the two laps took the corner flat, which is
    // not a difference of zero and must not read as one.
    let or_blank = |value: Option<String>| value.unwrap_or_else(|| "—".to_string());

    Row::new(vec![
        format!("T{}", zone.number),
        // What braking cost here, which is the entry-to-apex part and not the
        // whole section: charging an exit to the brakes sends a driver to fix
        // something that was not broken.
        format!(
            "{:+.2}",
            zone.entry_loss_ms.unwrap_or(zone.delta_ms) as f32 / 1000.0
        ),
        or_blank(zone.brake_delta_m.map(|m| format!("{m:+.0} m"))),
        or_blank(zone.peak_decel_g.map(|g| format!("{g:.2} g"))),
        or_blank(zone.reference_decel_g.map(|g| format!("{g:.2} g"))),
        or_blank(zone.trail_delta_ms.map(|ms| format!("{ms:+} ms"))),
        or_blank(zone.min_speed_delta.map(|kmh| format!("{kmh:+.1}"))),
        zone.fault
            .map(|fault| fault.label().to_string())
            .unwrap_or_default(),
    ])
    .style(Style::default().fg(colour))
}
