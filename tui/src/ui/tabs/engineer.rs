use crate::AppState;
use crate::ui::localization::tr;
use ac_core::analyzer::LapData;
use ac_core::config::Language;
use ratatui::{prelude::*, widgets::*};

pub struct EngineerState {
    pub active_sub_tab: usize,
}

impl EngineerState {
    pub fn new() -> Self {
        Self { active_sub_tab: 0 }
    }

    pub fn next_tab(&mut self) {
        self.active_sub_tab = (self.active_sub_tab + 1) % 2;
    }

    pub fn prev_tab(&mut self) {
        self.active_sub_tab = (self.active_sub_tab + 1) % 2;
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

    if app.ui_state.engineer.active_sub_tab == 0 {
        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(layout[1]);

        render_live_recs(f, content_layout[0], app);
        render_stats(f, content_layout[1], app);
    } else {
        render_debrief(f, layout[1], app);
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

    if app.ui_state.engineer.active_sub_tab == 0 {
        let content_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[1]);

        render_live_recs(f, content_layout[0], app);
        render_stats(f, content_layout[1], app);
    } else {
        render_debrief(f, layout[1], app);
    }
}

fn render_sub_tabs(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;
    let lang = &app.config.language;
    let is_ru = *lang == Language::Russian;

    let titles = vec![
        if is_ru {
            "🔴 РЕАЛЬНОЕ ВРЕМЯ [<-]"
        } else {
            "🔴 LIVE FEED [<-]"
        },
        if is_ru {
            "📋 ДЕБРИФИНГ [->]"
        } else {
            "📋 POST-STINT [->]"
        },
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
        .block(Block::default().title(if is_ru {
            "Плавность (Smoothness)"
        } else {
            "Smoothness"
        }))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent(style.smoothness.clamp(0.0, 100.0) as u16);
    f.render_widget(smooth_gauge, layout[0]);

    let aggr_gauge = Gauge::default()
        .block(Block::default().title(if is_ru {
            "Агрессия (Aggression)"
        } else {
            "Aggression"
        }))
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
        .percent(style.aggression.clamp(0.0, 100.0) as u16);
    f.render_widget(aggr_gauge, layout[1]);

    let trail_gauge = Gauge::default()
        .block(Block::default().title(if is_ru {
            "Трейл-брейкинг (Trail Braking)"
        } else {
            "Trail Braking"
        }))
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::DarkGray))
        .percent(style.trail_braking.clamp(0.0, 100.0) as u16);
    f.render_widget(trail_gauge, layout[2]);

    let total_lockups = stats.lockup_frames_front + stats.lockup_frames_rear;

    let lockup_line = Line::from(vec![
        Span::styled(
            if is_ru {
                "🛑 Блокировки колес: "
            } else {
                "🛑 Lockups detected: "
            },
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
            if is_ru {
                "🌀 Пробуксовки/Спины: "
            } else {
                "🌀 Wheelspin/Spins: "
            },
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
    let selected_idx = app
        .ui_state
        .setup_list_state
        .selected()
        .unwrap_or(default_idx)
        .min(default_idx);
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
    let title = if is_ru {
        " СВОДКА КРУГА (ВВЕРХ/ВНИЗ) "
    } else {
        " LAP SUMMARY (UP/DOWN) "
    };
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
            Span::styled(
                if is_ru { "КРУГ " } else { "LAP " },
                Style::default().fg(Color::DarkGray),
            ),
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
        lines.push(Line::from(if is_ru {
            "Нет данных. Проедьте круг."
        } else {
            "No data available. Drive a lap."
        }));
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
        .title(if is_ru {
            " ИНЖЕНЕРНЫЙ АНАЛИЗ И ТЕЛЕМЕТРИЯ "
        } else {
            " ENGINEER ANALYSIS & TELEMETRY "
        });

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if let Some(lap) = lap_opt {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(inner_area);

        let alerts = &app.config.alerts;
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

        let fl_rh = lap.avg_ride_height[0] * 1000.0;
        let fr_rh = lap.avg_ride_height[0] * 1000.0;
        let rl_rh = lap.avg_ride_height[1] * 1000.0;
        let rr_rh = lap.avg_ride_height[1] * 1000.0;

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
        let is_bottoming = fl_rh < 15.0 || fr_rh < 15.0 || rl_rh < 15.0 || rr_rh < 15.0;

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
                    format!(" [{:>4.1} psi] ", fl_psi),
                    Style::default().fg(fl_psi_c).add_modifier(Modifier::BOLD),
                ),
                Span::raw("                "),
                Span::styled(
                    format!(" [{:>4.1} psi] ", fr_psi),
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
                    format!(" (B: {:>3.0}°C) ", fl_brake),
                    Style::default().fg(fl_brake_c),
                ),
                Span::raw("                "),
                Span::styled(
                    format!(" (B: {:>3.0}°C) ", fr_brake),
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
                    format!(" (B: {:>3.0}°C) ", rl_brake),
                    Style::default().fg(rl_brake_c),
                ),
                Span::raw("                "),
                Span::styled(
                    format!(" (B: {:>3.0}°C) ", rr_brake),
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
                    format!(" [{:>4.1} psi] ", rl_psi),
                    Style::default().fg(rl_psi_c).add_modifier(Modifier::BOLD),
                ),
                Span::raw("                "),
                Span::styled(
                    format!(" [{:>4.1} psi] ", rr_psi),
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

        let mut lines = Vec::new();

        let mk_tag = |text: &'static str, color: Color| {
            Span::styled(
                text,
                Style::default()
                    .fg(Color::Black)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            )
        };

        let ok_tag = mk_tag(if is_ru { " OK " } else { " OK " }, Color::Green);
        let warn_tag = mk_tag(
            if is_ru {
                " ВНИМАНИЕ "
            } else {
                " WARNING "
            },
            Color::Yellow,
        );
        let crit_tag = mk_tag(
            if is_ru {
                " КРИТИЧНО "
            } else {
                " CRITICAL "
            },
            Color::Red,
        );

        lines.push(Line::from(Span::styled(
            if is_ru {
                "📡 АЭРОДИНАМИКА И КЛИРЕНС"
            } else {
                "📡 AERO & RIDE HEIGHT"
            },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("——————————————————————————————————————"));

        if is_bottoming {
            lines.push(Line::from(vec![
                crit_tag.clone(),
                Span::styled(
                    if is_ru {
                        " УДАРЫ ДНИЩЕМ О ТРАССУ"
                    } else {
                        " BOTTOMING OUT DETECTED"
                    },
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Увеличьте клиренс (Ride Height) или жесткость Packer."
                } else {
                    "   >> [ADVICE]: Increase Ride Height or Packer thickness."
                },
                Style::default().fg(Color::Yellow),
            )));
        }

        if is_oversteering {
            lines.push(Line::from(vec![
                warn_tag.clone(),
                Span::styled(
                    if is_ru {
                        " ИЗБЫТОЧНАЯ ПОВОРАЧИВАЕМОСТЬ (Занос)"
                    } else {
                        " OVERSTEER DETECTED"
                    },
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Увеличьте заднее антикрыло (Rear Wing +)."
                } else {
                    "   >> [ADVICE]: Increase REAR WING downforce."
                },
                Style::default().fg(Color::Gray),
            )));
        } else if is_understeering {
            lines.push(Line::from(vec![
                warn_tag.clone(),
                Span::styled(
                    if is_ru {
                        " НЕДОСТАТОЧНАЯ ПОВОРАЧИВАЕМОСТЬ (Снос)"
                    } else {
                        " UNDERSTEER DETECTED"
                    },
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Увеличьте передний сплиттер (Front Splitter +)."
                } else {
                    "   >> [ADVICE]: Increase FRONT SPLITTER downforce."
                },
                Style::default().fg(Color::Gray),
            )));
        } else if !is_bottoming {
            lines.push(Line::from(vec![
                ok_tag.clone(),
                Span::styled(
                    if is_ru {
                        " Аэродинамический баланс в норме."
                    } else {
                        " Aero balance is optimal."
                    },
                    Style::default().fg(Color::Green),
                ),
            ]));
        }
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            if is_ru {
                "🌡️ ШИНЫ (РАЗВАЛ/ДАВЛЕНИЕ) И ТОРМОЗА"
            } else {
                "🌡️ TYRES (CAMBER/PSI) & BRAKES"
            },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("——————————————————————————————————————"));

        let front_camber_diff: f32 = (fl_temp_i - fl_temp_o).abs();
        if front_camber_diff > 12.0 {
            lines.push(Line::from(vec![
                warn_tag.clone(),
                Span::styled(
                    if is_ru {
                        format!(
                            " Сильный градиент температуры ({:.0}°C).",
                            front_camber_diff
                        )
                    } else {
                        format!(" High tyre temp gradient ({:.0}°C).", front_camber_diff)
                    },
                    Style::default().fg(Color::Yellow),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Уменьшите отрицательный развал (Camber)."
                } else {
                    "   >> [ADVICE]: Reduce negative Camber."
                },
                Style::default().fg(Color::Gray),
            )));
        } else if front_camber_diff < 4.0 {
            lines.push(Line::from(vec![
                warn_tag.clone(),
                Span::styled(
                    if is_ru {
                        " Шина прогрета слишком равномерно."
                    } else {
                        " Tyre heated too evenly."
                    },
                    Style::default().fg(Color::Yellow),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Увеличьте отрицательный развал (Camber) для зацепа."
                } else {
                    "   >> [ADVICE]: Add negative Camber for cornering grip."
                },
                Style::default().fg(Color::Gray),
            )));
        } else {
            lines.push(Line::from(vec![
                ok_tag.clone(),
                Span::styled(
                    if is_ru {
                        " Развал (Camber) настроен оптимально."
                    } else {
                        " Camber angles are optimal."
                    },
                    Style::default().fg(Color::Green),
                ),
            ]));
        }

        let max_brake_temp = fl_brake.max(fr_brake);
        let min_brake_temp = rl_brake.min(rr_brake);

        if max_brake_temp > (target_brake_temp + 150.0) {
            lines.push(Line::from(vec![
                crit_tag.clone(),
                Span::styled(
                    if is_ru {
                        " КРИТИЧЕСКИЙ ПЕРЕГРЕВ ТОРМОЗОВ!"
                    } else {
                        " CRITICAL BRAKE OVERHEAT!"
                    },
                    Style::default().fg(Color::Red),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Откройте воздуховоды (Brake Ducts +)."
                } else {
                    "   >> [ADVICE]: Open Brake Ducts immediately."
                },
                Style::default().fg(Color::Yellow),
            )));
        } else if min_brake_temp < (target_brake_temp - 200.0) {
            lines.push(Line::from(vec![
                warn_tag.clone(),
                Span::styled(
                    if is_ru {
                        " Тормоза переохлаждены."
                    } else {
                        " Brakes are overcooled."
                    },
                    Style::default().fg(Color::Yellow),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Закройте воздуховоды (Brake Ducts -)."
                } else {
                    "   >> [ADVICE]: Close Brake Ducts."
                },
                Style::default().fg(Color::Gray),
            )));
        }

        let avg_psi = (fl_psi + fr_psi + rl_psi + rr_psi) / 4.0;
        let target_psi_diff = (avg_psi - target_psi).abs();
        if target_psi_diff > 0.4 {
            lines.push(Line::from(vec![
                crit_tag.clone(),
                Span::styled(
                    if is_ru {
                        format!(
                            " Давление не в окне (дельта: {:+.1} psi).",
                            avg_psi - target_psi
                        )
                    } else {
                        format!(
                            " Pressures out of window (delta: {:+.1} psi).",
                            avg_psi - target_psi
                        )
                    },
                    Style::default().fg(Color::Red),
                ),
            ]));
        }

        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            if is_ru {
                "🧠 ЭЛЕКТРОНИКА (ABS/TC) И ПОДВЕСКА"
            } else {
                "🧠 ELECTRONICS (ABS/TC) & SUSPENSION"
            },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("——————————————————————————————————————"));

        if lap.lockup_count > 2 {
            lines.push(Line::from(vec![
                crit_tag.clone(),
                Span::styled(
                    if is_ru {
                        format!(" Блокировки колес ({} раз)!", lap.lockup_count)
                    } else {
                        format!(" Wheel lockups ({} times)!", lap.lockup_count)
                    },
                    Style::default().fg(Color::Red),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Увеличьте ABS (+1) или сместите баланс тормозов назад."
                } else {
                    "   >> [ADVICE]: Increase ABS (+1) or move Brake Bias rearwards."
                },
                Style::default().fg(Color::Yellow),
            )));
        } else {
            lines.push(Line::from(vec![
                ok_tag.clone(),
                Span::styled(
                    if is_ru {
                        " Блокировок не обнаружено."
                    } else {
                        " No lockups detected."
                    },
                    Style::default().fg(Color::Green),
                ),
            ]));
        }

        let front_rh_diff: f32 = (fl_rh - fr_rh).abs();
        let rear_rh_diff: f32 = (rl_rh - rr_rh).abs();

        if front_rh_diff > 5.0 || rear_rh_diff > 5.0 {
            lines.push(Line::from(vec![
                warn_tag.clone(),
                Span::styled(
                    if is_ru {
                        " Асимметрия подвески в поворотах."
                    } else {
                        " High suspension roll asymmetry."
                    },
                    Style::default().fg(Color::Yellow),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                if is_ru {
                    "   >> [СОВЕТ]: Сделайте стабилизаторы (ARB) жестче для стабильности."
                } else {
                    "   >> [ADVICE]: Stiffen Anti-Roll Bars (ARB) for stability."
                },
                Style::default().fg(Color::Gray),
            )));
        }

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), layout[1]);
    }
}
