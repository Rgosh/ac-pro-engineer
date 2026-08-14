use crate::AppState;
use crate::ui::localization::tr;
use ac_core::analyzer::LapData;
use ac_core::config::Language;
use ac_core::i18n::Translate;
use ratatui::{prelude::*, widgets::*};

/// Live feed, post-stint debrief, and pressures.
const SUB_TAB_COUNT: usize = 3;

pub struct EngineerState {
    pub active_sub_tab: usize,
    pub selected_lap_index: usize,
}

impl Default for EngineerState {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineerState {
    pub fn new() -> Self {
        Self {
            active_sub_tab: 0,
            selected_lap_index: 0,
        }
    }

    pub fn next_tab(&mut self) {
        self.active_sub_tab = (self.active_sub_tab + 1) % SUB_TAB_COUNT;
    }

    pub fn prev_tab(&mut self) {
        self.active_sub_tab = (self.active_sub_tab + SUB_TAB_COUNT - 1) % SUB_TAB_COUNT;
    }

    pub fn next_lap(&mut self, total_laps: usize) {
        if total_laps > 0 && self.selected_lap_index + 1 < total_laps {
            self.selected_lap_index += 1;
        }
    }

    pub fn prev_lap(&mut self) {
        if self.selected_lap_index > 0 {
            self.selected_lap_index -= 1;
        }
    }
}

pub fn render_horizontal(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = main_block.inner(area);
    f.render_widget(main_block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);

    render_sub_tabs(f, layout[0], app);

    match app.ui_state.engineer.active_sub_tab {
        0 => {
            let content_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(layout[1]);

            render_live_recs(f, content_layout[0], app);
            render_stats(f, content_layout[1], app);
        }
        1 => render_debrief(f, layout[1], app),
        _ => render_pressures(f, layout[1], app),
    }
}

pub fn render_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = main_block.inner(area);
    f.render_widget(main_block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);

    render_sub_tabs(f, layout[0], app);

    match app.ui_state.engineer.active_sub_tab {
        0 => {
            let content_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(layout[1]);

            render_live_recs(f, content_layout[0], app);
            render_stats(f, content_layout[1], app);
        }
        1 => render_debrief(f, layout[1], app),
        _ => render_pressures(f, layout[1], app),
    }
}

fn render_sub_tabs(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;

    let titles = vec![
        "🔴 LIVE FEED [<-]".tr(is_ru),
        "📋 POST-STINT".tr(is_ru),
        "🎯 PRESSURES [->]".tr(is_ru),
    ];

    let tabs = Tabs::new(titles)
        .select(app.ui_state.engineer.active_sub_tab)
        .block(Block::default().borders(Borders::BOTTOM))
        .style(Style::default().fg(app.ui_state.get_color(&theme.text)))
        .highlight_style(
            Style::default()
                .fg(app.ui_state.get_color(&theme.highlight))
                .add_modifier(Modifier::BOLD),
        )
        .divider(" | ");

    f.render_widget(tabs, area);
}

fn render_live_recs(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;

    let block = Block::default()
        .title(tr("eng_recs", lang))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let recs: Vec<ListItem<'_>> = app
        .recommendations
        .iter()
        .map(|r| {
            let (color, icon) = match r.severity {
                ac_core::engineer::Severity::Critical => (Color::Red, "🚨"),
                ac_core::engineer::Severity::Warning => (Color::Yellow, "⚠️"),
                _ => (Color::Green, "ℹ️"),
            };

            ListItem::new(vec![Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(
                    r.message.clone(),
                    Style::default().fg(app.ui_state.get_color(&theme.text)),
                ),
            ])])
        })
        .collect();

    let list = List::new(recs).block(block);
    f.render_widget(list, area);
}

fn render_stats(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;

    let block = Block::default()
        .title(tr("eng_analysis", lang))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner_area);

    let stats = &app.engineer.stats;
    let style = &app.engineer.driving_style;

    let smooth_gauge = Gauge::default()
        .block(Block::default().title("Smoothness".tr(is_ru)))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent(style.smoothness.clamp(0.0, 100.0) as u16);
    f.render_widget(smooth_gauge, layout[0]);

    let aggr_gauge = Gauge::default()
        .block(Block::default().title("Aggression".tr(is_ru)))
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
        .percent(style.aggression.clamp(0.0, 100.0) as u16);
    f.render_widget(aggr_gauge, layout[1]);

    let trail_gauge = Gauge::default()
        .block(Block::default().title("Trail Braking".tr(is_ru)))
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::DarkGray))
        .percent(style.trail_braking.clamp(0.0, 100.0) as u16);
    f.render_widget(trail_gauge, layout[2]);

    let total_lockups = stats.lockup_frames_front + stats.lockup_frames_rear;

    let lockup_line = Line::from(vec![
        Span::styled(
            "🛑 Lockups detected: ".tr(is_ru),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            total_lockups.to_string(),
            Style::default()
                .fg(if total_lockups > 0 {
                    Color::Red
                } else {
                    Color::Green
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(lockup_line), layout[4]);

    let spin_line = Line::from(vec![
        Span::styled(
            "🌀 Wheelspin/Spins: ".tr(is_ru),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            stats.wheel_spin_frames.to_string(),
            Style::default()
                .fg(if stats.wheel_spin_frames > 0 {
                    Color::Red
                } else {
                    Color::Green
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(spin_line), layout[5]);
}

fn render_debrief(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    let total_laps = app.analyzer.laps.len();
    let default_idx = total_laps.saturating_sub(1);
    let selected_idx = app.ui_state.engineer.selected_lap_index.min(default_idx);
    let lap = app.analyzer.laps.get(selected_idx);

    render_debrief_header(f, layout[0], app, lap, total_laps, selected_idx, is_ru);
    render_sector_advice(f, layout[1], app, lap, is_ru);
}

fn render_debrief_header(
    f: &mut Frame<'_>,
    area: Rect,
    _app: &AppState,
    lap_opt: Option<&LapData>,
    total_laps: usize,
    cur_idx: usize,
    is_ru: bool,
) {
    let title = " LAP SUMMARY (UP/DOWN) ".tr(is_ru);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .title_alignment(Alignment::Center);

    let mut lines = Vec::new();
    if let Some(lap) = lap_opt {
        let min = lap.lap_time_ms / 60000;
        let sec = (lap.lap_time_ms % 60000) / 1000;
        let ms = lap.lap_time_ms % 1000;

        lines.push(Line::from(vec![
            Span::styled("LAP ".tr(is_ru), Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("#{} / {}", cur_idx + 1, total_laps),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  │  "),
            Span::styled(
                format!("{}:{:02}.{:03}", min, sec, ms),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  │  "),
            Span::styled(
                if lap.valid {
                    "✅ VALID"
                } else {
                    "❌ INVALID"
                },
                Style::default()
                    .fg(if lap.valid { Color::Green } else { Color::Red })
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled(
                format!("🚀 MAX {:.1} km/h", lap.max_speed),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  │  "),
            Span::styled(
                format!("⛽ USED {:.2} L", lap.fuel_used),
                Style::default().fg(Color::Magenta),
            ),
        ]));
    } else {
        lines.push(Line::from("No data available. Drive a lap.".tr(is_ru)));
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center),
        area,
    );
}

fn render_sector_advice(
    f: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    lap_opt: Option<&LapData>,
    is_ru: bool,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" ENGINEER ANALYSIS & TELEMETRY ".tr(is_ru));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if let Some(lap) = lap_opt {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(inner_area);

        let alerts = &app.config.alerts;
        let fmt = app.config.formatter();
        let target_psi = (alerts.tyre_pressure_min + alerts.tyre_pressure_max) / 2.0;
        let target_brake_temp = (alerts.brake_temp_max - 150.0).max(300.0);

        let fl_psi = lap.avg_wheels_pressure[0];
        let fr_psi = lap.avg_wheels_pressure[1];
        let rl_psi = lap.avg_wheels_pressure[2];
        let rr_psi = lap.avg_wheels_pressure[3];

        let fl_temp_i = lap.avg_tyre_temp_i[0];
        let fl_temp_m = lap.avg_tyre_temp_m[0];
        let fl_temp_o = lap.avg_tyre_temp_o[0];

        let fr_temp_i = lap.avg_tyre_temp_i[1];
        let fr_temp_m = lap.avg_tyre_temp_m[1];
        let fr_temp_o = lap.avg_tyre_temp_o[1];

        let rl_temp_i = lap.avg_tyre_temp_i[2];
        let rl_temp_m = lap.avg_tyre_temp_m[2];
        let rl_temp_o = lap.avg_tyre_temp_o[2];

        let rr_temp_i = lap.avg_tyre_temp_i[3];
        let rr_temp_m = lap.avg_tyre_temp_m[3];
        let rr_temp_o = lap.avg_tyre_temp_o[3];

        let fl_brake = lap.avg_brake_temp[0];
        let fr_brake = lap.avg_brake_temp[1];
        let rl_brake = lap.avg_brake_temp[2];
        let rr_brake = lap.avg_brake_temp[3];

        // AC publishes ride height per axle, not per corner — AcPhysics
        // carries [front, rear] and nothing more. Both corners of an axle
        // therefore show the same number; there is no per-corner measurement
        // to display, and none to compare.
        let front_rh = lap.avg_ride_height[0] * 1000.0;
        let rear_rh = lap.avg_ride_height[1] * 1000.0;
        let (fl_rh, fr_rh) = (front_rh, front_rh);
        let (rl_rh, rr_rh) = (rear_rh, rear_rh);

        let get_status_color = |val: f32, target: f32, tolerance: f32| -> Color {
            let diff = (val - target).abs();
            if diff <= tolerance {
                Color::Green
            } else if diff <= tolerance * 2.0 {
                Color::Yellow
            } else {
                Color::Red
            }
        };

        let fl_psi_c = get_status_color(fl_psi, target_psi, 0.3);
        let fr_psi_c = get_status_color(fr_psi, target_psi, 0.3);
        let rl_psi_c = get_status_color(rl_psi, target_psi, 0.3);
        let rr_psi_c = get_status_color(rr_psi, target_psi, 0.3);

        let fl_brake_c = get_status_color(fl_brake, target_brake_temp, 150.0);
        let fr_brake_c = get_status_color(fr_brake, target_brake_temp, 150.0);
        let rl_brake_c = get_status_color(rl_brake, target_brake_temp, 150.0);
        let rr_brake_c = get_status_color(rr_brake, target_brake_temp, 150.0);

        let is_oversteering = lap.oversteer_count > lap.understeer_count && lap.oversteer_count > 2;
        let is_understeering =
            lap.understeer_count > lap.oversteer_count && lap.understeer_count > 2;
        // The bottoming check moved into `ac_core::debrief`, where it reads
        // `avg_ride_height` — [front, rear] — rather than four numbers of which
        // each pair is the same by construction. The car visual below still
        // colours the wing and splitter from the balance counts.

        let car_body_style = Style::default().fg(Color::DarkGray);
        let wheel_style = Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD);

        let rear_wing_style = if is_oversteering {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            car_body_style
        };
        let front_splitter_style = if is_understeering {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            car_body_style
        };

        let car_visual = vec![
            Line::from(vec![
                Span::styled(
                    format!(" [{:>6}] ", fmt.format_pressure(fl_psi)),
                    Style::default().fg(fl_psi_c).add_modifier(Modifier::BOLD),
                ),
                Span::raw("              "),
                Span::styled(
                    format!(" [{:>6}] ", fmt.format_pressure(fr_psi)),
                    Style::default().fg(fr_psi_c).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(
                        " [{:>2.0}|{:>2.0}|{:>2.0}] ",
                        fl_temp_i, fl_temp_m, fl_temp_o
                    ),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw("                "),
                Span::styled(
                    format!(
                        " [{:>2.0}|{:>2.0}|{:>2.0}] ",
                        fr_temp_o, fr_temp_m, fr_temp_i
                    ),
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" (B: {:>5}) ", fmt.format_temp(fl_brake)),
                    Style::default().fg(fl_brake_c),
                ),
                Span::raw("              "),
                Span::styled(
                    format!(" (B: {:>5}) ", fmt.format_temp(fr_brake)),
                    Style::default().fg(fr_brake_c),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" ↕ {:>2.0}mm  ", fl_rh),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled("   [||]", wheel_style),
                Span::styled("==========", front_splitter_style),
                Span::styled("[||]   ", wheel_style),
                Span::styled(
                    format!("  ↕ {:>2.0}mm", fr_rh),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![Span::styled(
                "               \\   ____   /               ",
                car_body_style,
            )]),
            Line::from(vec![Span::styled(
                "                | /    \\ |                ",
                car_body_style,
            )]),
            Line::from(vec![Span::styled(
                "                || (  ) ||                ",
                car_body_style,
            )]),
            Line::from(vec![Span::styled(
                "                ||      ||                ",
                car_body_style,
            )]),
            Line::from(vec![Span::styled(
                "                | \\____/ |                ",
                car_body_style,
            )]),
            Line::from(vec![Span::styled(
                "               /          \\               ",
                car_body_style,
            )]),
            Line::from(vec![
                Span::styled(
                    format!(" ↕ {:>2.0}mm  ", rl_rh),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled("   [||]", wheel_style),
                Span::styled("----------", car_body_style),
                Span::styled("[||]   ", wheel_style),
                Span::styled(
                    format!("  ↕ {:>2.0}mm", rr_rh),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled("               ", car_body_style),
                Span::styled("[==========]", rear_wing_style),
                Span::styled("               ", car_body_style),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" (B: {:>5}) ", fmt.format_temp(rl_brake)),
                    Style::default().fg(rl_brake_c),
                ),
                Span::raw("              "),
                Span::styled(
                    format!(" (B: {:>5}) ", fmt.format_temp(rr_brake)),
                    Style::default().fg(rr_brake_c),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(
                        " [{:>2.0}|{:>2.0}|{:>2.0}] ",
                        rl_temp_i, rl_temp_m, rl_temp_o
                    ),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw("                "),
                Span::styled(
                    format!(
                        " [{:>2.0}|{:>2.0}|{:>2.0}] ",
                        rr_temp_o, rr_temp_m, rr_temp_i
                    ),
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" [{:>6}] ", fmt.format_pressure(rl_psi)),
                    Style::default().fg(rl_psi_c).add_modifier(Modifier::BOLD),
                ),
                Span::raw("              "),
                Span::styled(
                    format!(" [{:>6}] ", fmt.format_pressure(rr_psi)),
                    Style::default().fg(rr_psi_c).add_modifier(Modifier::BOLD),
                ),
            ]),
        ];

        let car_layout_center = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(5),
                Constraint::Min(0),
                Constraint::Percentage(5),
            ])
            .split(layout[0]);

        f.render_widget(
            Paragraph::new(car_visual).alignment(Alignment::Center),
            car_layout_center[1],
        );

        // The whole of this column used to be computed here — three hundred
        // lines where the analysis and the spans that draw it were the same
        // code. It is `ac_core::debrief` now, which is the same function the
        // in-game panel renders, so the terminal and the overlay cannot give
        // different advice about the same lap. They could, and did: the camber
        // verdict here threw away the sign of the temperature spread and told a
        // car short of camber to take camber out.
        let advice = ac_core::debrief::debrief(lap, &app.config);

        let mut lines = Vec::new();

        let mk_tag = |severity: &ac_core::engineer::Severity| {
            let (label, color) = match *severity {
                ac_core::engineer::Severity::Critical => (" CRIT ", Color::Red),
                ac_core::engineer::Severity::Warning => (" WARN ", Color::Yellow),
                ac_core::engineer::Severity::Info => (" INFO ", Color::Cyan),
            };
            Span::styled(
                label,
                Style::default()
                    .fg(Color::Black)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            )
        };

        for rec in &advice {
            let confidence = rec.confidence_level();
            lines.push(Line::from(vec![
                mk_tag(&rec.severity),
                // How sure, before the sentence rather than after it. An
                // engineer that says the same thing about one odd frame and
                // about four consistent corners is not an engineer, and the
                // marker is what lets a driver tell those apart at a glance.
                Span::styled(
                    format!(" {} ", confidence.marker()),
                    Style::default().fg(confidence_colour(confidence)),
                ),
                Span::styled(rec.message.clone(), Style::default().fg(Color::White)),
            ]));
            // The chain, where the rule can state one: why it happened, and
            // what to look at next time to know whether the change worked.
            if let Some(chain) = rec.chain.as_ref() {
                if !chain.cause.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("   {} {}", "cause:".tr(is_ru), chain.cause),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                if !chain.confirm.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("   {} {}", "confirm:".tr(is_ru), chain.confirm),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
            if !rec.action.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("   >> {}", rec.action),
                    Style::default().fg(Color::Gray),
                )));
            }
        }

        // Nothing wrong is worth saying out loud. A blank column reads as "the
        // analysis has not run" rather than as "the lap was clean".
        if advice.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    "  OK  ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " Nothing to report — the lap was clean.".tr(is_ru),
                    Style::default().fg(Color::Green),
                ),
            ]));
        }

        push_stint_verdicts(&mut lines, app, is_ru);

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), layout[1]);
    }
}

/// Green for a finding backed by several agreeing observations, red for one
/// the analysis is not willing to stand behind yet.
fn confidence_colour(confidence: ac_core::confidence::Confidence) -> Color {
    use ac_core::confidence::Confidence;
    match confidence {
        Confidence::High => Color::Green,
        Confidence::Medium => Color::Yellow,
        Confidence::Low => Color::Red,
    }
}

/// What the stint says about the car versus the driving.
///
/// Under the lap's advice because it answers a different question: the advice
/// above is about the lap that just ended, this is about the run as a whole,
/// and it deliberately refuses to answer until it has enough of one.
fn push_stint_verdicts(lines: &mut Vec<Line<'_>>, app: &AppState, is_ru: bool) {
    use ac_core::driver_vs_car::{Assessment, Blame};

    // This session's laps, not everything the analyser is holding. A ghost
    // loaded from a file was driven on another day in another car, and counting
    // it is how a verdict about *this* car gets made from somebody else's lap.
    let laps: Vec<ac_core::analyzer::LapData> = app
        .analyzer
        .laps
        .iter()
        .filter(|lap| !lap.from_file)
        .cloned()
        .collect();

    let assessment = ac_core::driver_vs_car::assess(&laps);
    let heading = |lines: &mut Vec<Line<'_>>| {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "OVER THE STINT — THE CAR OR THE DRIVING".tr(is_ru),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
    };

    match assessment {
        // The refusal is drawn, not hidden. A blank space here reads as the
        // analysis being broken; "four more laps" reads as an engineer who
        // will not guess.
        Assessment::NotYet(not_yet) => {
            heading(lines);
            lines.push(Line::from(Span::styled(
                if is_ru {
                    format!(
                        "  Кругов {} из {} — одного круга мало, чтобы отличить машину от пилотажа.",
                        not_yet.laps, not_yet.needed
                    )
                } else {
                    format!(
                        "  {} of {} laps — one lap cannot tell the car from the driving.",
                        not_yet.laps, not_yet.needed
                    )
                },
                Style::default().fg(Color::DarkGray),
            )));
        }
        Assessment::Verdicts(verdicts) if verdicts.is_empty() => {}
        Assessment::Verdicts(verdicts) => {
            heading(lines);
            for verdict in verdicts.iter().take(3) {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", verdict.confidence.marker()),
                        Style::default().fg(confidence_colour(verdict.confidence)),
                    ),
                    Span::styled(
                        format!("{:<14}", verdict.symptom),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        verdict.blame.label(is_ru),
                        Style::default()
                            .fg(match verdict.blame {
                                Blame::Car => Color::Yellow,
                                Blame::Driver => Color::Cyan,
                                Blame::Undecided => Color::DarkGray,
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::from(Span::styled(
                    format!("     {}", verdict.reason),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }
}

/// Tyre pressure targets: what to set cold to arrive at the hot target, and
/// what the current corner temperatures say to change.
///
/// `ColdPressureCalculator` and `TyrePressureOptimizer` were both fully
/// implemented in `ac_core::engineer` and referenced only from the test suite.
fn render_pressures(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    use ac_core::engineer::{ColdPressureCalculator, TyrePressureOptimizer};

    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == Language::Russian;
    let fmt = app.config.formatter();

    let block = Block::default()
        .title(" PRESSURE TARGETS ".tr(is_ru))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(phys) = app.ac_physics() else {
        f.render_widget(
            Paragraph::new("Waiting for telemetry...".tr(is_ru))
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };

    let gfx = app.ac_graphics();
    let ambient = phys.air_temp;
    let grip = gfx.map(|g| g.surface_grip).unwrap_or(1.0);

    let mut lines = Vec::new();

    // Cold targets, front and rear, from the configured hot targets.
    lines.push(Line::from(Span::styled(
        "COLD SETUP PRESSURES".tr(is_ru),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        format!(
            "{} {:.0}°  |  {} {:.2}",
            "Air".tr(is_ru),
            fmt.temp_val(ambient),
            "grip".tr(is_ru),
            grip
        ),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    for (label, target) in [
        ("Front".tr(is_ru), app.config.target_hot_pressure_front),
        ("Rear".tr(is_ru), app.config.target_hot_pressure_rear),
    ] {
        let estimate = ColdPressureCalculator::calculate(target, ambient, grip);
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<6} ", label),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>7}", fmt.format_pressure(estimate.recommended_cold_psi)),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  → {} {}",
                    fmt.format_pressure(estimate.target_hot_psi),
                    "hot".tr(is_ru)
                ),
                Style::default().fg(Color::Gray),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            format!(
                "         -{:.1} {}  -{:.1} {}",
                estimate.delta_temp_psi,
                "temp".tr(is_ru),
                estimate.delta_grip_psi,
                "grip|short".tr(is_ru)
            ),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "PER-CORNER ADJUSTMENT".tr(is_ru),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let optimizer = TyrePressureOptimizer::calculate(phys, app.config.target_tyre_pressure);
    for corner in &optimizer.corners {
        let delta = corner.recommended_delta_psi;
        let (delta_text, delta_color) = if delta.abs() < 0.05 {
            ("  ok".tr(is_ru).to_string(), Color::Green)
        } else if delta > 0.0 {
            (format!("+{:.1}", delta), Color::Yellow)
        } else {
            (format!("{:.1}", delta), Color::LightBlue)
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<4}", corner.corner_name),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{:>7}  ", fmt.format_pressure(corner.current_psi))),
            Span::styled(
                format!("{:>5}", delta_text),
                Style::default()
                    .fg(delta_color)
                    .add_modifier(Modifier::BOLD),
            ),
            // Inner-minus-outer is a difference, so it is scaled but never
            // offset — a 32°F shift does not belong to a temperature delta.
            Span::styled(
                format!(
                    "   Δ{:>5.1}{}",
                    fmt.temp_delta_val(corner.temp_spread_c),
                    fmt.temp_symbol()
                ),
                Style::default().fg(if corner.temp_spread_c.abs() > 12.0 {
                    Color::Red
                } else {
                    Color::DarkGray
                }),
            ),
        ]));
    }

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
