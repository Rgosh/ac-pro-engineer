use crate::AppState;
use ac_core::i18n::Translate;
use ratatui::{prelude::*, widgets::*};

/// Coerce a value into the 0.0..=1.0 that `Gauge::ratio` and
/// `LineGauge::ratio` assert on.
///
/// `clamp` alone is not enough: it returns NaN for NaN, and the assert then
/// fires on it. Every ratio in this app is ultimately a division of two floats
/// read out of shared memory, so a stale or zeroed page — the state a leaked
/// `/dev/shm` mapping leaves behind — reaches these call sites as NaN and
/// takes the whole app down on the next frame.
pub fn safe_ratio(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The gear a driver would call it, from [`Car::gear`](ac_core::games::Car).
///
/// One function because there were three copies of this, and a boundary
/// convention kept in three places is one that gets changed in two. The
/// reading counts reverse as −1 and neutral as 0 — the game's own numbering,
/// whatever it is, is translated in the game's folder.
pub fn gear_label(gear: i32) -> String {
    match gear {
        -1 => "R".to_string(),
        0 => "N".to_string(),
        n => n.to_string(),
    }
}

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

/// Tyre wear color. AC convention: 100% = new tyre, 0% = fully worn.
pub fn get_wear_color(wear: f32) -> Color {
    match wear {
        w if w >= 96.0 => Color::Green,      // Excellent condition
        w if w >= 80.0 => Color::LightGreen, // Good
        w if w >= 60.0 => Color::Yellow,     // Monitor
        w if w >= 40.0 => Color::LightRed,   // Worn — consider pitting
        _ => Color::Red,                     // Critical — pit now
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
    if let Some(data) = app.car() {
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

        let fmt = app.config.formatter();
        let pressure = data.tyre_pressure_psi[index];
        let pressure_text = fmt.format_pressure(pressure);
        let pressure_widget = Paragraph::new(pressure_text)
            .style(Style::default().fg(get_pressure_color(pressure)))
            .alignment(Alignment::Center);

        // What the game measures decides what these two rows say. Every
        // number here used to be printed whatever the game published, which on
        // Competizione drew `I0 M0 O0` and `0.0%` in red — a tyre with no
        // tread left and no heat in it, on a car that was fine.
        let measures = app
            .reading
            .as_ref()
            .map(|reading| reading.capabilities)
            .unwrap_or_else(ac_core::games::Capabilities::all);

        let avg_temp = data.avg_tyre_temp_c(index);
        let temp_text = if measures.tyre_edge_temps {
            format!(
                "I{:.0} M{:.0} O{:.0}",
                fmt.temp_val(data.tyre_temp_inner_c[index]),
                fmt.temp_val(data.tyre_temp_middle_c[index]),
                fmt.temp_val(data.tyre_temp_outer_c[index])
            )
        } else {
            // The core, and labelled as the core: it is a real reading of the
            // same tyre taken further in, not a third of the tread.
            format!("core {:.0}", fmt.temp_val(avg_temp))
        };
        let temp_widget = Paragraph::new(temp_text)
            .style(Style::default().fg(get_tyre_color(avg_temp)))
            .alignment(Alignment::Center);

        // Tyre wear where a game reports it, and what is left of the brake pad
        // where it reports that instead — which is the trade Competizione
        // makes, and the one number a GT3 stint is actually decided by.
        let wear = data.tyre_wear[index];
        let (wear_text, wear_colour) = if measures.tyre_wear {
            (format!("{wear:.1}%"), get_wear_color(wear))
        } else if measures.brake_wear {
            let pad = data.brake_pad_mm[index];
            (
                format!("pad {pad:.1}mm"),
                // Onto the wear colours' usable band — see
                // `dashboard::pad_on_the_wear_scale`, which is where the two
                // numbers live so the two screens cannot disagree.
                get_wear_color(crate::ui::tabs::dashboard::pad_on_the_wear_scale(app, pad)),
            )
        } else {
            ("—".to_string(), Color::DarkGray)
        };
        let wear_widget = Paragraph::new(wear_text)
            .style(Style::default().fg(wear_colour))
            .alignment(Alignment::Center);

        let brake_temp = data.brake_temp_c[index];
        let brake_text = format!("B{}", fmt.format_temp(brake_temp));
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

    if let Some(data) = app.car() {
        let speed_block = Block::default()
            .title("SPD".tr_lang(lang).to_string())
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
            .title("RPM".tr_lang(lang).to_string())
            .borders(Borders::ALL);
        // max_rpm is 0 until the static page has been read, so this division
        // has to be guarded — otherwise the ratio is inf and the readout sits
        // pegged at the redline colour before the car has even been loaded.
        // `render_header` has the same guard.
        let rpm_ratio = if app.session_info.max_rpm > 0 {
            data.rpm as f32 / app.session_info.max_rpm as f32
        } else {
            0.0
        };
        let rpm = Paragraph::new(format!("{}\nRPM", data.rpm))
            .style(
                Style::default()
                    .fg(get_rpm_color(rpm_ratio))
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(rpm_block);
        f.render_widget(rpm, layout[1]);

        let gear = gear_label(data.gear);
        let gear_block = Block::default()
            .title("GEAR".tr_lang(lang).to_string())
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
            .title("DELTA".tr_lang(lang).to_string())
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

/// The keys that do something on the tab on screen, bottom right.
///
/// Every word of this comes from `keys::hints` and `keys::describe`, which
/// read the same bindings `keys::resolve` acts on — so it cannot advertise a
/// key that does nothing, which is what the Setup tab's hand-typed
/// `'D' - Download` did on a screen with no D handler.
///
/// Right-aligned on the status row, and dropped entirely when the terminal is
/// too narrow to hold it beside the chips: half a hint is worse than none.
pub fn render_tab_hints(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let is_ru = app.config.language == ac_core::config::Language::Russian;

    let parts: Vec<String> = crate::keys::hints(app.active_tab)
        .iter()
        .filter_map(|(field, english, russian)| {
            let binding = crate::keys::value_of(&app.config.keys, field)?;
            // A binding nobody can read is a key nobody can press. Left out
            // rather than printed, so the promise this line makes stays true.
            crate::keys::parse(binding)?;
            Some(format!(
                "{} {}",
                crate::keys::describe(binding),
                if is_ru { russian } else { english }
            ))
        })
        .collect();

    if parts.is_empty() {
        return;
    }

    let text = format!(" {} ", parts.join("  ·  "));
    let width = text.chars().count() as u16;
    if width + 2 >= area.width {
        return;
    }

    let hint_area = Rect {
        x: area.x + area.width - width,
        y: area.y,
        width,
        height: 1,
    };

    f.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::Gray).bg(Color::Reset)),
        hint_area,
    );
}

#[cfg(test)]
mod tests {
    use super::{gear_label, safe_ratio};

    /// Reverse and neutral are the two the reading renumbered, and the two
    /// every screen used to decode for itself.
    #[test]
    fn the_gear_label_reads_the_readings_numbering() {
        assert_eq!(gear_label(-1), "R");
        assert_eq!(gear_label(0), "N");
        assert_eq!(gear_label(1), "1");
        assert_eq!(gear_label(6), "6");
    }

    #[test]
    fn safe_ratio_passes_values_already_in_range() {
        assert_eq!(safe_ratio(0.0), 0.0);
        assert_eq!(safe_ratio(0.5), 0.5);
        assert_eq!(safe_ratio(1.0), 1.0);
    }

    #[test]
    fn safe_ratio_clamps_out_of_range_values() {
        assert_eq!(safe_ratio(-3.0), 0.0);
        assert_eq!(safe_ratio(42.0), 1.0);
    }

    /// The case `clamp` alone does not cover: it returns NaN unchanged, and
    /// ratatui's `ratio` assert then fires on it.
    #[test]
    fn safe_ratio_converts_non_finite_values_to_zero() {
        assert_eq!(safe_ratio(f64::NAN), 0.0);
        assert_eq!(safe_ratio(f64::INFINITY), 0.0);
        assert_eq!(safe_ratio(f64::NEG_INFINITY), 0.0);
        // How this actually arises: a ratio taken against a field that is
        // still zero because shared memory has not been read yet.
        let unread_from_shm = 0.0_f64;
        assert_eq!(safe_ratio(unread_from_shm / unread_from_shm), 0.0);
    }
}
