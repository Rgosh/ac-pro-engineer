use ac_core::config::Language;
use ac_core::overlay::OverlayMode;
use ac_tui::ui::UIRenderer;
use ac_tui::{AppStage, AppState, AppTab, SafeLock};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;
use std::path::Path;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn buffer_to_svg(
    buffer: &ratatui::buffer::Buffer,
    width: u16,
    height: u16,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let char_w = 10u32;
    let char_h = 20u32;
    let font_size = 14u32;
    let img_w = (width as u32) * char_w;
    let img_h = (height as u32) * char_h;

    let mut svg = String::with_capacity(64 * 1024);
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}" style="background-color: #0f1419; font-family: 'Consolas', 'Fira Code', 'DejaVu Sans Mono', monospace; font-size: {}px;">"#,
        img_w, img_h, img_w, img_h, font_size
    ));
    svg.push('\n');

    // 1. Draw Cell Backgrounds
    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            let bg_color = match cell.bg {
                ratatui::style::Color::DarkGray => "#282d37",
                ratatui::style::Color::Gray => "#3c414b",
                ratatui::style::Color::Blue => "#1e3a8a",
                ratatui::style::Color::Red => "#991b1b",
                ratatui::style::Color::Green => "#166534",
                ratatui::style::Color::Yellow => "#854d0e",
                ratatui::style::Color::Cyan => "#155e75",
                ratatui::style::Color::Magenta => "#701a75",
                _ => continue,
            };

            let px = (x as u32) * char_w;
            let py = (y as u32) * char_h;
            svg.push_str(&format!(
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                px, py, char_w, char_h, bg_color
            ));
            svg.push('\n');
        }
    }

    // 2. Draw Cell Text Glyphs
    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            let symbol = cell.symbol();
            if symbol.is_empty() || symbol == " " {
                continue;
            }

            let fg_color = match cell.fg {
                ratatui::style::Color::Red => "#f87171",
                ratatui::style::Color::Green => "#4ade80",
                ratatui::style::Color::Yellow => "#facc15",
                ratatui::style::Color::Blue => "#60a5fa",
                ratatui::style::Color::Magenta => "#c084fc",
                ratatui::style::Color::Cyan => "#38bdf8",
                ratatui::style::Color::Gray => "#9ca3af",
                ratatui::style::Color::DarkGray => "#6b7280",
                ratatui::style::Color::White => "#f3f4f6",
                _ => "#e2e8f0",
            };

            let px = (x as u32) * char_w + 1;
            let py = (y as u32) * char_h + 15;

            let escaped = escape_xml(symbol);
            svg.push_str(&format!(
                r#"  <text x="{}" y="{}" fill="{}">{}</text>"#,
                px, py, fg_color, escaped
            ));
            svg.push('\n');
        }
    }

    svg.push_str("</svg>\n");

    fs::write(output_path, svg)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting SVG Vector Screenshot Generator (14 target screens)...");

    let width = 140;
    let height = 40;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppState::new(OverlayMode::External);
    app.config.language = Language::English;
    let renderer = UIRenderer::new();

    let screenshot_dir = Path::new("screenshots");
    if !screenshot_dir.exists() {
        fs::create_dir_all(screenshot_dir)?;
    }

    // 1. Launcher.svg
    app.stage = AppStage::Launcher;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Launcher.svg"),
    )?;
    println!("  [1/14] Rendered Launcher.svg");

    // 2. Main Running Stage
    app.stage = AppStage::Running;

    let targets = [
        (AppTab::Dashboard, "Dashboard.svg"),
        (AppTab::Telemetry, "Telemetry.svg"),
        (AppTab::Engineer, "Engineer.svg"),
        (AppTab::Setup, "Setup_1.svg"),
        (AppTab::Analysis, "Analysis_Overview.svg"),
        (AppTab::Strategy, "Strategy.svg"),
        (AppTab::Ffb, "FFB_Tuning.svg"),
        (AppTab::Settings, "Settings.svg"),
        (AppTab::Guide, "Guide.svg"),
    ];

    for (tab, filename) in &targets {
        app.active_tab = *tab;
        terminal.draw(|f| renderer.render(f, &app))?;
        buffer_to_svg(
            terminal.backend().buffer(),
            width,
            height,
            &screenshot_dir.join(filename),
        )?;
        println!("  [OK] Rendered {}", filename);
    }

    // 3. Setup_cloud.svg
    app.active_tab = AppTab::Setup;
    *app.setup_manager.browser_active.safe_lock() = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Setup_cloud.svg"),
    )?;
    *app.setup_manager.browser_active.safe_lock() = false;
    println!("  [OK] Rendered Setup_cloud.svg");

    // 4. Analysis_Radar.svg
    app.active_tab = AppTab::Analysis;
    app.ui_state.analysis.next_tab();
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Analysis_Radar.svg"),
    )?;
    println!("  [OK] Rendered Analysis_Radar.svg");

    // 5. Help_Modal.svg
    app.ui_state.show_help = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Help_Modal.svg"),
    )?;
    app.ui_state.show_help = false;
    println!("  [OK] Rendered Help_Modal.svg");

    // 6. Overlay_Control.svg
    app.ui_state.overlay_mode = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Overlay_Control.svg"),
    )?;
    app.ui_state.overlay_mode = false;
    println!("  [OK] Rendered Overlay_Control.svg");

    println!("\nALL 14 VECTOR SVG SCREENSHOTS GENERATED SUCCESSFULLY!");
    Ok(())
}
