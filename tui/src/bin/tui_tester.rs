use ac_core::config::Language;
use ac_core::overlay::OverlayMode;
use ac_tui::ui::{overlay, UIRenderer};
use ac_tui::{AppStage, AppState, AppTab};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;
use std::path::Path;

fn buffer_to_svg(buffer: &ratatui::buffer::Buffer, width: u16, height: u16, title: &str) -> String {
    let char_width = 8;
    let char_height = 16;
    let svg_w = width as usize * char_width;
    let svg_h = height as usize * char_height + 30;

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" style=\"background-color: #0f1419; font-family: monospace; font-size: 13px;\">",
        svg_w, svg_h
    ));
    svg.push_str(&format!(
        "<rect width=\"100%\" height=\"30\" fill=\"#1a202c\"/><text x=\"10\" y=\"20\" fill=\"#6366f1\" font-weight=\"bold\">RaceEngineer TUI Visual Test: {}</text>",
        title
    ));

    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            let ch = cell.symbol();
            if ch == " " {
                continue;
            }
            let px = x as usize * char_width + 5;
            let py = y as usize * char_height + 45;

            let fg_color = match cell.fg {
                ratatui::style::Color::Red => "#f87171",
                ratatui::style::Color::Green => "#4ade80",
                ratatui::style::Color::Yellow => "#facc15",
                ratatui::style::Color::Blue => "#60a5fa",
                ratatui::style::Color::Magenta => "#c084fc",
                ratatui::style::Color::Cyan => "#38bdf8",
                ratatui::style::Color::Gray | ratatui::style::Color::DarkGray => "#9ca3af",
                ratatui::style::Color::White => "#f3f4f6",
                _ => "#e2e8f0",
            };

            let escaped_ch = match ch {
                "&" => "&amp;",
                "<" => "&lt;",
                ">" => "&gt;",
                "\"" => "&quot;",
                "'" => "&apos;",
                other => other,
            };

            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" fill=\"{}\">{}</text>",
                px, py, fg_color, escaped_ch
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

fn buffer_to_text(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            text.push_str(cell.symbol());
        }
        text.push('\n');
    }
    text
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting RaceEngineer TUI Menu & Action Test Runner...");

    let width = 140;
    let height = 40;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppState::new(OverlayMode::External);
    let renderer = UIRenderer::new();

    let screenshot_dir = Path::new("screenshots");
    if !screenshot_dir.exists() {
        fs::create_dir_all(screenshot_dir)?;
    }

    // 1. Test Launcher Stage
    app.stage = AppStage::Launcher;
    terminal.draw(|f| renderer.render(f, &app))?;
    let svg = buffer_to_svg(terminal.backend().buffer(), width, height, "Launcher");
    let txt = buffer_to_text(terminal.backend().buffer(), width, height);
    fs::write(screenshot_dir.join("test_stage_launcher.svg"), svg)?;
    fs::write(screenshot_dir.join("test_stage_launcher.txt"), txt)?;
    println!("  [OK] Tested Stage Launcher");

    // 2. Test Running Stage
    app.stage = AppStage::Running;

    let tabs = [
        (AppTab::Dashboard, "Dashboard"),
        (AppTab::Telemetry, "Telemetry"),
        (AppTab::Engineer, "Engineer"),
        (AppTab::Setup, "Setup"),
        (AppTab::Analysis, "Analysis"),
        (AppTab::Strategy, "Strategy"),
        (AppTab::Ffb, "FFB_Tuning"),
        (AppTab::Settings, "Settings"),
        (AppTab::Guide, "Guide"),
    ];

    // 3. Test All Tabs in English
    app.config.language = Language::English;
    for (tab, name) in &tabs {
        app.active_tab = *tab;
        terminal.draw(|f| renderer.render(f, &app))?;
        let svg = buffer_to_svg(
            terminal.backend().buffer(),
            width,
            height,
            &format!("Tab - {} (EN)", name),
        );
        let txt = buffer_to_text(terminal.backend().buffer(), width, height);
        fs::write(
            screenshot_dir.join(format!("test_tab_{}_en.svg", name.to_lowercase())),
            svg,
        )?;
        fs::write(
            screenshot_dir.join(format!("test_tab_{}_en.txt", name.to_lowercase())),
            txt,
        )?;
        println!("  [OK] Tested Tab {} (English)", name);
    }

    // 4. Test All Tabs in Russian
    app.config.language = Language::Russian;
    for (tab, name) in &tabs {
        app.active_tab = *tab;
        terminal.draw(|f| renderer.render(f, &app))?;
        let svg = buffer_to_svg(
            terminal.backend().buffer(),
            width,
            height,
            &format!("Tab - {} (RU)", name),
        );
        let txt = buffer_to_text(terminal.backend().buffer(), width, height);
        fs::write(
            screenshot_dir.join(format!("test_tab_{}_ru.svg", name.to_lowercase())),
            svg,
        )?;
        fs::write(
            screenshot_dir.join(format!("test_tab_{}_ru.txt", name.to_lowercase())),
            txt,
        )?;
        println!("  [OK] Tested Tab {} (Russian)", name);
    }

    // 5. Test Overlay Control Center Popup Modal
    app.show_overlay_menu = true;
    terminal.draw(|f| {
        renderer.render(f, &app);
        overlay::render(f, f.size(), &app);
    })?;
    let svg = buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        "Overlay Menu Modal",
    );
    let txt = buffer_to_text(terminal.backend().buffer(), width, height);
    fs::write(screenshot_dir.join("test_modal_overlay_menu.svg"), svg)?;
    fs::write(screenshot_dir.join("test_modal_overlay_menu.txt"), txt)?;
    app.show_overlay_menu = false;
    println!("  [OK] Tested Overlay Menu Modal");

    // 6. Test Help Modal
    app.show_help = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    let svg = buffer_to_svg(terminal.backend().buffer(), width, height, "Help Modal");
    let txt = buffer_to_text(terminal.backend().buffer(), width, height);
    fs::write(screenshot_dir.join("test_modal_help.svg"), svg)?;
    fs::write(screenshot_dir.join("test_modal_help.txt"), txt)?;
    app.show_help = false;
    println!("  [OK] Tested Help Modal");

    println!("\nALL TUI MENU & ACTION TESTS PASSED SUCCESSFULLY!");
    println!("Visual screenshots saved to screenshots/ directory.");

    Ok(())
}
