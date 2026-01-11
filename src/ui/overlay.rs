use crate::ui::widgets::*;
use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let height = area.height.min(5);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(height)])
        .split(area);

    let overlay_area = if area.height < 10 { area } else { chunks[1] };

    let block = Block::default()
        .borders(Borders::TOP)
        .style(Style::default().bg(Color::Black));

    f.render_widget(block, overlay_area);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(35),
        ])
        .split(overlay_area.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        }));

    if let Some(phys) = &app.physics_mem {
        let p = phys.get();

        let gear_char = match p.gear {
            0 => "R",
            1 => "N",
            n => return render_gear_num(f, layout[0], n - 1, p.rpms, app),
        };
        render_gear_text(f, layout[0], gear_char, Color::Red);

        let speed_text = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{:.0}", p.speed_kmh),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled("KM/H", Style::default().fg(Color::DarkGray))),
        ])
        .alignment(Alignment::Center);
        f.render_widget(speed_text, layout[1]);

        let delta = app.engineer.stats.current_delta;
        let d_color = get_delta_color(delta);
        let delta_text = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{:+.3}", delta),
                Style::default().fg(d_color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled("DELTA", Style::default().fg(Color::Gray))),
        ])
        .alignment(Alignment::Center);
        f.render_widget(delta_text, layout[2]);

        let fuel_text = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("{:.1} L", p.fuel),
                Style::default().fg(get_fuel_color(app.engineer.stats.fuel_laps_remaining)),
            )),
            Line::from(Span::styled(
                format!("~{:.1} laps", app.engineer.stats.fuel_laps_remaining),
                Style::default().fg(Color::White),
            )),
        ])
        .alignment(Alignment::Right);
        f.render_widget(fuel_text, layout[3]);
    } else {
        let wait = Paragraph::new("WAITING FOR GAME...").alignment(Alignment::Center);
        f.render_widget(wait, overlay_area);
    }
}

fn render_gear_num(f: &mut Frame<'_>, area: Rect, gear: i32, rpm: i32, app: &AppState) {
    let max_rpm = app.session_info.max_rpm;
    let color = if rpm > max_rpm - 200 {
        Color::Red
    } else {
        Color::Yellow
    };

    let p = Paragraph::new(format!("{}", gear))
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    f.render_widget(p, area);
}

fn render_gear_text(f: &mut Frame<'_>, area: Rect, text: &str, color: Color) {
    let p = Paragraph::new(text)
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    f.render_widget(p, area);
}

pub fn format_time(ms: i32) -> String {
    let sign = if ms < 0 { "-" } else { "" };
    let abs_ms = ms.abs();
    let minutes = abs_ms / 60000;
    let seconds = (abs_ms % 60000) / 1000;
    let millis = abs_ms % 1000;
    format!("{}{}:{:02}.{:03}", sign, minutes, seconds, millis)
}
