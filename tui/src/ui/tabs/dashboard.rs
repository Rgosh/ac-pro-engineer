use crate::AppState;
use ac_core::i18n::Translate;
use ratatui::{prelude::*, widgets::*};

/// Draw a titled panel explaining that there is no telemetry yet.
///
/// Better than the early `return` these call sites used to do, which drew
/// literally nothing and left an unexplained hole in the layout.
fn render_waiting_panel(f: &mut Frame<'_>, area: Rect, app: &AppState, title: &str) {
    let theme = &app.ui_state.theme;
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let message = if app.is_game_running {
        "Waiting for telemetry from Assetto Corsa...".tr(is_ru)
    } else {
        "Assetto Corsa is not running".tr(is_ru)
    };

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    f.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(block),
        area,
    );
}

pub fn render_horizontal(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    render_tyre_panel(f, layout[0], app);
    render_central_panel(f, layout[1], app);
    render_info_panel(f, layout[2], app);
}

pub fn render_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(area);

    render_tyres_vertical(f, layout[0], app);
    render_quick_info_vertical(f, layout[1], app);
}

fn render_tyre_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title("TYRE MONITOR".tr_lang(lang).to_string())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(inner);

    let front = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(layout[0]);
    render_tyre_widget(f, front[0], 0, app, "FL");
    render_tyre_widget(f, front[1], 1, app, "FR");

    let rear = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(layout[1]);
    render_tyre_widget(f, rear[0], 2, app, "RL");
    render_tyre_widget(f, rear[1], 3, app, "RR");

    // Accessor again, so the summary populates under --demo too.
    if let Some(data) = app.car() {
        let avg_pressure: f32 = data.tyre_pressure_psi.iter().sum::<f32>() / 4.0;
        let avg_temp: f32 = (0..4).map(|i| data.avg_tyre_temp_c(i)).sum::<f32>() / 4.0;

        let fmt = app.config.formatter();
        let summary_text = vec![
            Line::from(vec![
                Span::styled("Avg Press: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    fmt.format_pressure(avg_pressure),
                    Style::default().fg(get_pressure_color(avg_pressure)),
                ),
            ]),
            Line::from(vec![
                Span::styled("Avg Temp:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    fmt.format_temp(avg_temp),
                    Style::default().fg(get_tyre_color(avg_temp)),
                ),
            ]),
        ];

        let summary_block = Block::default()
            .borders(Borders::TOP)
            .padding(Padding::new(1, 0, 0, 0));
        f.render_widget(Paragraph::new(summary_text).block(summary_block), layout[2]);
    }
}

fn render_central_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;

    // Through the accessors, not `app.mem` directly: they fall back to the
    // mock data, which is why the whole cockpit was blank under --demo. And
    // an early `return` drawing nothing at all is indistinguishable from a
    // rendering bug, so say what is going on instead.
    let (Some(phys), Some(gfx)) = (app.car(), app.session()) else {
        render_waiting_panel(f, area, app, "COCKPIT");
        return;
    };
    let (phys, gfx) = (*phys, *gfx);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" COCKPIT ")
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .split(inner);

    let max_rpm = if app.session_info.max_rpm > 0 {
        app.session_info.max_rpm as f32
    } else {
        8000.0
    };
    let rpm_ratio = (phys.rpm as f32 / max_rpm).clamp(0.0, 1.0);

    let (rpm_color, label_text) = if rpm_ratio > 0.96 {
        (Color::Blue, "SHIFT NOW!".to_string())
    } else if rpm_ratio > 0.90 {
        (Color::Red, format!("{} RPM", phys.rpm))
    } else if rpm_ratio > 0.75 {
        (Color::Yellow, format!("{} RPM", phys.rpm))
    } else {
        (Color::Green, format!("{} RPM", phys.rpm))
    };

    let gauge_style = if rpm_ratio > 0.96 {
        Style::default()
            .fg(Color::White)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(rpm_color).bg(Color::DarkGray)
    };

    f.render_widget(
        LineGauge::default()
            .ratio(crate::ui::widgets::safe_ratio(rpm_ratio as f64))
            .label(label_text)
            .gauge_style(gauge_style),
        layout[0],
    );

    let main_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(layout[1]);

    let gear_char = crate::ui::widgets::gear_label(phys.gear);

    let speed_block = Block::default().borders(Borders::RIGHT);
    let speed_p = Paragraph::new(vec![
        Line::from(Span::styled(
            format!("{:.0}", phys.speed_kmh),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::ITALIC),
        )),
        Line::from(Span::styled("km/h", Style::default().fg(Color::DarkGray))),
    ])
    .alignment(Alignment::Center)
    .block(speed_block);

    let gear_p = Paragraph::new(gear_char)
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().padding(Padding::new(0, 0, 1, 0)));

    f.render_widget(speed_p, main_row[0]);
    f.render_widget(gear_p, main_row[1]);

    let pedals_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(layout[2]);

    render_mini_bar(f, pedals_layout[0], "C", phys.clutch, Color::Blue);
    render_mini_bar(f, pedals_layout[1], "B", phys.brake, Color::Red);
    render_mini_bar(f, pedals_layout[2], "T", phys.throttle, Color::Green);

    let elec_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(layout[3]);

    let row1 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(elec_layout[0]);
    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(elec_layout[1]);

    let tc_level = phys.tc_level;
    let abs_level = phys.abs_level;
    let tc_cut = gfx.tc_cut;
    let map_level = gfx.engine_map;
    let bias = phys.brake_bias * 100.0;

    let tc_active = phys.tc_in_action > 0.0;
    let abs_active = phys.abs_in_action > 0.0;

    let tc_enabled_phys = phys.tc > 0.0;
    let abs_enabled_phys = phys.abs > 0.0;

    render_status_tile(f, row1[0], "TC", tc_level, tc_enabled_phys, tc_active);
    render_status_tile(f, row1[1], "ABS", abs_level, abs_enabled_phys, abs_active);

    if tc_cut > 0 {
        render_simple_tile(f, row2[0], "TC CUT", format!("{}", tc_cut), Color::Cyan);
    } else {
        render_simple_tile(f, row2[0], "MAP", format!("{}", map_level), Color::Magenta);
    }

    render_simple_tile(f, row2[1], "BIAS", format!("{:.1}%", bias), Color::Cyan);
}

fn render_status_tile(
    f: &mut Frame<'_>,
    area: Rect,
    label: &str,
    level: i32,
    enabled_phys: bool,
    active: bool,
) {
    let (text, fg, bg) = if active {
        (
            if level > 0 {
                format!("{}", level)
            } else {
                "ACT".to_string()
            },
            Color::Black,
            Color::Yellow,
        )
    } else if level > 0 {
        (format!("{}", level), Color::Green, Color::Reset)
    } else if enabled_phys {
        ("ON".to_string(), Color::Green, Color::Reset)
    } else {
        ("OFF".to_string(), Color::DarkGray, Color::Reset)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(bg));

    let p = Paragraph::new(vec![
        Line::from(Span::styled(
            label,
            Style::default()
                .fg(if active { Color::Black } else { Color::Gray })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            text,
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(Alignment::Center)
    .block(block);

    f.render_widget(p, area);
}

fn render_simple_tile(f: &mut Frame<'_>, area: Rect, label: &str, value: String, color: Color) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let p = Paragraph::new(vec![
        Line::from(Span::styled(label, Style::default().fg(Color::Gray))),
        Line::from(Span::styled(
            value,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(Alignment::Center)
    .block(block);

    f.render_widget(p, area);
}

fn render_mini_bar(f: &mut Frame<'_>, area: Rect, label: &str, val: f32, color: Color) {
    let gauge = LineGauge::default()
        .block(Block::default().padding(Padding::new(1, 1, 0, 0)))
        .gauge_style(Style::default().fg(color))
        .ratio(crate::ui::widgets::safe_ratio(val as f64))
        .label(label);
    f.render_widget(gauge, area);
}

fn render_info_panel(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let (Some(phys), Some(gfx)) = (app.car(), app.session()) else {
        render_waiting_panel(f, area, app, "SESSION");
        return;
    };

    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let block = Block::default()
        .title("SESSION INFO".tr_lang(lang).to_string())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let list = vec![
        Line::from(vec![
            Span::styled(
                format!("{}: ", "Car".tr_lang(lang)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                &app.session_info.car_name,
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", "Track|as the terminal abbreviates it".tr_lang(lang)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                &app.session_info.track_name,
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Time Left: ", Style::default().fg(Color::Gray)),
            Span::styled(
                ac_core::session_info::SessionTiming::format_time_left_minutes(
                    gfx.session_time_left_ms,
                ),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("{}: ", "Fuel|as the terminal abbreviates it".tr_lang(lang)),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{:.1} L", phys.fuel_litres),
                Style::default().fg(get_fuel_color(app.engineer.stats.fuel_laps_remaining)),
            ),
        ]),
    ];

    f.render_widget(
        Paragraph::new(list).block(Block::default().padding(Padding::new(1, 1, 1, 1))),
        inner,
    );
}

fn render_tyres_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    render_tyre_panel(f, area, app);
}

fn render_quick_info_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    render_central_panel(f, area, app);
}

/// Brake pad thickness on the same scale the tyre-wear colours speak.
///
/// They run from the driver's own critical threshold to a fresh tyre, not from
/// zero — 85 % is "change it" by default — so millimetres mapped onto a plain
/// percentage would show a pad with two thirds of its life left as spent.
pub fn pad_on_the_wear_scale(app: &AppState, pad_mm: f32) -> f32 {
    /// A GT3 pad starts around here.
    const NEW_MM: f32 = 29.0;
    /// ...and this is a stop rather than a plan. Same numbers the engineer's
    /// brake-wear rule uses.
    const SPENT_MM: f32 = 8.0;

    let critical = app.config.alerts.wear_critical.clamp(0.0, 99.0);
    let left = ((pad_mm - SPENT_MM) / (NEW_MM - SPENT_MM)).clamp(0.0, 1.0);
    critical + left * (100.0 - critical)
}

fn render_tyre_widget(f: &mut Frame<'_>, area: Rect, idx: usize, app: &AppState, label: &str) {
    if let Some(data) = app.car() {
        let temp = data.avg_tyre_temp_c(idx);
        let press = data.tyre_pressure_psi[idx];

        let block = Block::default().borders(Borders::ALL).title(label);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let fmt = app.config.formatter();
        // The third line is whichever consumable the game measures. It was
        // always tyre wear, which Competizione does not publish — so all four
        // corners drew "0%" in red, a set with nothing left, on a car that had
        // just gone out. That game measures what is left of the brake pad
        // instead, and for a GT3 stint that is the number that decides the
        // race.
        let measures = app
            .reading
            .as_ref()
            .map(|reading| reading.capabilities)
            .unwrap_or_else(ac_core::games::Capabilities::all);
        let third_line = if measures.tyre_wear {
            let wear = data.tyre_wear[idx];
            Span::styled(
                format!("{wear:.0}%"),
                Style::default().fg(get_wear_color(wear)),
            )
        } else if measures.brake_wear {
            let pad = data.brake_pad_mm[idx];
            Span::styled(
                format!("{pad:.1}mm"),
                // Onto the tyre scale's *usable band* rather than onto 0..100:
                // the wear colours run from the driver's critical threshold to
                // a fresh tyre, so a raw percentage would paint a healthy set
                // of pads red at 20 mm. A GT3 pad starts near 29 mm and is a
                // stop below 8.
                Style::default().fg(get_wear_color(pad_on_the_wear_scale(app, pad))),
            )
        } else {
            Span::styled("—", Style::default().fg(Color::DarkGray))
        };

        let text = vec![
            Line::from(Span::styled(
                fmt.format_temp(temp),
                Style::default().fg(get_tyre_color(temp)),
            )),
            Line::from(Span::styled(
                fmt.format_pressure(press),
                Style::default().fg(get_pressure_color(press)),
            )),
            Line::from(third_line),
        ];
        f.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner);
    }
}

fn get_temp_color(temp: f32) -> Color {
    if temp < 70.0 {
        Color::Blue
    } else if temp > 100.0 {
        Color::Red
    } else {
        Color::Green
    }
}
fn get_tyre_color(temp: f32) -> Color {
    get_temp_color(temp)
}
fn get_pressure_color(press: f32) -> Color {
    if (press - 27.5).abs() < 1.5 {
        Color::Green
    } else {
        Color::Yellow
    }
}
fn get_wear_color(wear: f32) -> Color {
    super::super::widgets::get_wear_color(wear)
}
fn get_fuel_color(laps: f32) -> Color {
    if laps < 2.0 {
        Color::Red
    } else if laps < 5.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}
