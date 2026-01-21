use crate::ui::widgets::*;
use crate::AppState;
use ratatui::{
    prelude::*,
    widgets::{
        canvas::{Canvas, Line as CanvasLine},
        *,
    },
};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);

    if let Some(phys) = &app.physics_mem {
        let p = phys.get();

        let v_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(inner);

        render_rpm_bar(f, v_layout[0], p.rpms, app.session_info.max_rpm);

        let h_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(25),
                Constraint::Percentage(35),
                Constraint::Percentage(20),
            ])
            .split(v_layout[1]);

        render_gear_speed(
            f,
            h_layout[0],
            p.gear,
            p.speed_kmh,
            p.rpms,
            app.session_info.max_rpm,
        );

        render_delta_panel(f, h_layout[1], app.engineer.stats.current_delta);

        render_g_force_diagram(f, h_layout[2], p.acc_g);

        render_status_panel(f, h_layout[3], p, &app.engineer.stats);

        render_inputs(f, v_layout[2], p.gas, p.brake);
    } else {
        let wait = Paragraph::new("WAITING FOR ASSETTO CORSA...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(wait, inner);
    }
}

fn render_rpm_bar(f: &mut Frame<'_>, area: Rect, rpm: i32, max_rpm: i32) {
    let max = if max_rpm > 0 { max_rpm as f32 } else { 8000.0 };
    let ratio = (rpm as f32 / max).clamp(0.0, 1.0);

    let color = if ratio > 0.96 {
        Color::Blue
    } else if ratio > 0.90 {
        Color::Red
    } else if ratio > 0.75 {
        Color::Yellow
    } else {
        Color::Green
    };

    let gauge = LineGauge::default()
        .ratio(ratio as f64)
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .line_set(symbols::line::THICK);

    f.render_widget(gauge, area);
}

fn render_gear_speed(f: &mut Frame<'_>, area: Rect, gear: i32, speed: f32, rpm: i32, max_rpm: i32) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let gear_char = match gear {
        0 => "R".to_string(),
        1 => "N".to_string(),
        n => format!("{}", n - 1),
    };

    let gear_color = if rpm > max_rpm - 200 {
        Color::Red
    } else {
        Color::Yellow
    };

    let gear_p = Paragraph::new(gear_char)
        .style(Style::default().fg(gear_color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);

    f.render_widget(gear_p, chunks[0]);

    let speed_p = Paragraph::new(format!("{:.0}", speed))
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);

    f.render_widget(speed_p, chunks[1]);
}

fn render_delta_panel(f: &mut Frame<'_>, area: Rect, delta: f32) {
    let color = if delta < -0.5 {
        Color::Magenta
    } else if delta < 0.0 {
        Color::Green
    } else if delta > 0.0 {
        Color::Red
    } else {
        Color::Gray
    };

    let p = Paragraph::new(vec![
        Line::from(Span::styled(
            "DELTA",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{:+.3}", delta),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(Alignment::Center)
    .block(Block::default().padding(Padding::new(0, 0, 1, 0)));

    f.render_widget(p, area);
}

fn render_g_force_diagram(f: &mut Frame<'_>, area: Rect, acc_g: [f32; 3]) {
    let canvas = Canvas::default()
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT))
        .x_bounds([-2.5, 2.5])
        .y_bounds([-2.5, 2.5])
        .paint(move |ctx| {
            ctx.draw(&CanvasLine {
                x1: -2.0,
                y1: 0.0,
                x2: 2.0,
                y2: 0.0,
                color: Color::DarkGray,
            });
            ctx.draw(&CanvasLine {
                x1: 0.0,
                y1: -2.0,
                x2: 0.0,
                y2: 2.0,
                color: Color::DarkGray,
            });

            let x = acc_g[0] as f64;
            let y = acc_g[2] as f64;

            let color = if y.abs() > 1.0 || x.abs() > 1.0 {
                Color::Red
            } else {
                Color::Yellow
            };

            ctx.print(x, y, Span::styled("●", Style::default().fg(color)));
        });

    f.render_widget(canvas, area);
}

fn render_status_panel(
    f: &mut Frame<'_>,
    area: Rect,
    p: &crate::ac_structs::AcPhysics,
    stats: &crate::engineer::EngineerStats,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    let fuel_color = if stats.fuel_laps_remaining < 2.0 {
        Color::Red
    } else {
        Color::Green
    };
    let fuel_p = Paragraph::new(format!("{:.1}L", p.fuel))
        .style(Style::default().fg(fuel_color))
        .alignment(Alignment::Right);
    f.render_widget(fuel_p, chunks[0]);

    let t_fl = p.get_avg_tyre_temp(0);
    let t_fr = p.get_avg_tyre_temp(1);
    let t_rl = p.get_avg_tyre_temp(2);
    let t_rr = p.get_avg_tyre_temp(3);

    let dot = "●";
    let tyres_line = Line::from(vec![
        Span::styled(dot, Style::default().fg(get_temp_color(t_fl))),
        Span::styled(dot, Style::default().fg(get_temp_color(t_fr))),
        Span::raw(" "),
        Span::styled(dot, Style::default().fg(get_temp_color(t_rl))),
        Span::styled(dot, Style::default().fg(get_temp_color(t_rr))),
    ]);

    let tyres_p = Paragraph::new(tyres_line).alignment(Alignment::Right);
    f.render_widget(tyres_p, chunks[1]);
}

fn render_inputs(f: &mut Frame<'_>, area: Rect, gas: f32, brake: f32) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let brake_gauge = LineGauge::default()
        .ratio(brake as f64)
        .gauge_style(Style::default().fg(Color::Red).bg(Color::Black))
        .line_set(symbols::line::THICK);
    f.render_widget(brake_gauge, chunks[0]);

    let gas_gauge = LineGauge::default()
        .ratio(gas as f64)
        .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
        .line_set(symbols::line::THICK);
    f.render_widget(gas_gauge, chunks[1]);
}

fn get_temp_color(temp: f32) -> Color {
    if temp < 75.0 {
        Color::Blue
    } else if temp > 105.0 {
        Color::Red
    } else {
        Color::Green
    }
}
