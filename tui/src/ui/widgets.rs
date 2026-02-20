use crate::ui::localization::tr;
use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn get_tyre_color(temp: f32) -> Color {
    match temp {
        t if t < 70.0 => Color::Blue,
        t if t < 85.0 => Color::Cyan,
        t if t < 95.0 => Color::Green,
        t if t < 105.0 => Color::Yellow,
        _ => Color::Red,
    }
}

pub fn get_pressure_color(psi: f32) -> Color {
    match psi {
        p if p < 26.0 => Color::Blue,
        p if p <= 27.5 => Color::Green,
        p if p <= 28.5 => Color::Yellow,
        _ => Color::Red,
    }
}

pub fn get_brake_color(temp: f32) -> Color {
    match temp {
        t if t < 300.0 => Color::Blue,
        t if t < 500.0 => Color::Green,
        t if t < 700.0 => Color::Yellow,
        _ => Color::Red,
    }
}

pub fn get_wear_color(wear: f32) -> Color {
    match wear {
        w if w < 30.0 => Color::Green,
        w if w < 60.0 => Color::Yellow,
        w if w < 80.0 => Color::LightRed,
        _ => Color::Red,
    }
}

pub fn get_rpm_color(rpm_percent: f32) -> Color {
    match rpm_percent {
        r if r < 0.7 => Color::Green,
        r if r < 0.85 => Color::Yellow,
        r if r < 0.95 => Color::LightRed,
        _ => Color::Red,
    }
}

pub fn get_delta_color(delta: f32) -> Color {
    match delta {
        d if d < -0.5 => Color::Magenta,
        d if d < -0.1 => Color::Green,
        d if d < 0.1 => Color::Yellow,
        d if d < 0.5 => Color::LightRed,
        _ => Color::Red,
    }
}

pub fn get_fuel_color(laps_remaining: f32) -> Color {
    match laps_remaining {
        l if l > 5.0 => Color::Green,
        l if l > 2.0 => Color::Yellow,
        l if l > 0.5 => Color::LightRed,
        _ => Color::Red,
    }
}

pub fn render_tyre_widget(
    f: &mut Frame<'_>,
    area: Rect,
    index: usize,
    app: &AppState,
    label: &str,
) {
    if let Some(phys) = &app.physics_mem {
        let data = phys.get();

        let theme = &app.ui_state.theme;
        let block = Block::default()
            .title(label)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)));

        let inner = block.inner(area);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        let pressure = data.wheels_pressure[index];
        let pressure_text = format!("{:.1} psi", pressure);
        let pressure_widget = Paragraph::new(pressure_text)
            .style(Style::default().fg(get_pressure_color(pressure)))
            .alignment(Alignment::Center);

        let temp_i = data.tyre_temp_i[index];
        let temp_m = data.tyre_temp_m[index];
        let temp_o = data.tyre_temp_o[index];
        let avg_temp = (temp_i + temp_m + temp_o) / 3.0;
        let temp_text = format!("I{:.0} M{:.0} O{:.0}", temp_i, temp_m, temp_o);
        let temp_widget = Paragraph::new(temp_text)
            .style(Style::default().fg(get_tyre_color(avg_temp)))
            .alignment(Alignment::Center);

        let wear = data.tyre_wear[index];
        let wear_text = format!("{:.1}%", wear);
        let wear_widget = Paragraph::new(wear_text)
            .style(Style::default().fg(get_wear_color(wear)))
            .alignment(Alignment::Center);

        let brake_temp = data.brake_temp[index];
        let brake_text = format!("B{:.0}°C", brake_temp);
        let brake_widget = Paragraph::new(brake_text)
            .style(Style::default().fg(get_brake_color(brake_temp)))
            .alignment(Alignment::Center);

        f.render_widget(pressure_widget, layout[0]);
        f.render_widget(temp_widget, layout[1]);
        f.render_widget(wear_widget, layout[2]);
        f.render_widget(brake_widget, layout[3]);
        f.render_widget(block, area);
    }
}

pub fn render_progress_bar(value: f32, max: f32) -> Span<'static> {
    let percent = (value / max * 100.0).min(100.0);
    let filled = (percent / 10.0).floor() as usize;
    let bar = "█".repeat(filled) + &"░".repeat(10 - filled);

    let color = if percent < 30.0 {
        Color::Red
    } else if percent < 70.0 {
        Color::Yellow
    } else {
        Color::Green
    };

    Span::styled(
        format!(" {:3.0}% {}", percent, bar),
        Style::default().fg(color),
    )
}

pub fn render_telemetry_bar_vertical(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lang = &app.config.language;
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Min(0),
        ])
        .split(area);

    if let Some(phys) = &app.physics_mem {
        let data = phys.get();

        let speed_block = Block::default()
            .title(tr("lbl_speed", lang))
            .borders(Borders::ALL);
        let speed = Paragraph::new(format!("{}\nkm/h", data.speed_kmh as i32))
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(speed_block);
        f.render_widget(speed, layout[0]);

        let rpm_block = Block::default()
            .title(tr("lbl_rpm", lang))
            .borders(Borders::ALL);
        let rpm = Paragraph::new(format!("{}\nRPM", data.rpms))
            .style(
                Style::default()
                    .fg(get_rpm_color(
                        data.rpms as f32 / app.session_info.max_rpm as f32,
                    ))
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(rpm_block);
        f.render_widget(rpm, layout[1]);

        let gear = match data.gear {
            0 => "R".to_string(),
            1 => "N".to_string(),
            n => format!("{}", n - 1),
        };
        let gear_block = Block::default()
            .title(tr("lbl_gear", lang))
            .borders(Borders::ALL);
        let gear_widget = Paragraph::new(gear)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(gear_block);
        f.render_widget(gear_widget, layout[2]);

        let delta = app.engineer.stats.current_delta;
        let delta_sign = if delta >= 0.0 { "+" } else { "" };
        let delta_block = Block::default()
            .title(tr("lbl_delta", lang))
            .borders(Borders::ALL);
        let delta_widget = Paragraph::new(format!("{}{:.3}", delta_sign, delta))
            .style(
                Style::default()
                    .fg(get_delta_color(delta))
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(delta_block);
        f.render_widget(delta_widget, layout[3]);
    }
}
