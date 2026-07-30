use ac_core::config::Language;
use ac_core::overlay::OverlayMode;
use ac_tui::ui::UIRenderer;
use ac_tui::{AppStage, AppState, AppTab, SafeLock};
use image::{ImageBuffer, Rgb};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;
use std::path::Path;

fn buffer_to_png(buffer: &ratatui::buffer::Buffer, width: u16, height: u16, output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let char_w = 9u32;
    let char_h = 18u32;
    let img_w = (width as u32) * char_w;
    let img_h = (height as u32) * char_h;

    let mut imgbuf: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_pixel(img_w, img_h, Rgb([15u8, 20u8, 25u8]));

    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            let ch = cell.symbol();
            if ch.is_empty() || ch == " " {
                let bg_color = match cell.bg {
                    ratatui::style::Color::DarkGray => Rgb([40u8, 45u8, 55u8]),
                    ratatui::style::Color::Gray => Rgb([60u8, 65u8, 75u8]),
                    ratatui::style::Color::Blue => Rgb([30u8, 58u8, 138u8]),
                    _ => Rgb([15u8, 20u8, 25u8]),
                };
                for px in (x as u32 * char_w)..((x as u32 + 1) * char_w) {
                    for py in (y as u32 * char_h)..((y as u32 + 1) * char_h) {
                        if px < img_w && py < img_h {
                            imgbuf.put_pixel(px, py, bg_color);
                        }
                    }
                }
                continue;
            }

            let fg_color = match cell.fg {
                ratatui::style::Color::Red => Rgb([248u8, 113u8, 113u8]),
                ratatui::style::Color::Green => Rgb([74u8, 222u8, 128u8]),
                ratatui::style::Color::Yellow => Rgb([250u8, 204u8, 21u8]),
                ratatui::style::Color::Blue => Rgb([96u8, 165u8, 250u8]),
                ratatui::style::Color::Magenta => Rgb([192u8, 132u8, 252u8]),
                ratatui::style::Color::Cyan => Rgb([56u8, 189u8, 248u8]),
                ratatui::style::Color::Gray => Rgb([156u8, 163u8, 175u8]),
                ratatui::style::Color::DarkGray => Rgb([107u8, 114u8, 128u8]),
                ratatui::style::Color::White => Rgb([243u8, 244u8, 246u8]),
                _ => Rgb([226u8, 232u8, 240u8]),
            };

            let start_x = (x as u32) * char_w;
            let start_y = (y as u32) * char_h;

            for px in (start_x + 1)..(start_x + char_w - 1) {
                for py in (start_y + 2)..(start_y + char_h - 2) {
                    if px < img_w && py < img_h {
                        imgbuf.put_pixel(px, py, fg_color);
                    }
                }
            }
        }
    }

    imgbuf.save(output_path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting English PNG Screenshot Generator...");

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

    // 1. Launcher.png
    app.stage = AppStage::Launcher;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_png(terminal.backend().buffer(), width, height, &screenshot_dir.join("Launcher.png"))?;
    println!("  [OK] Rendered Launcher.png");

    // 2. Main Running Stage (English Only)
    app.stage = AppStage::Running;

    let targets = [
        (AppTab::Dashboard, "Dashboard.png"),
        (AppTab::Telemetry, "Telemetry.png"),
        (AppTab::Engineer, "Engineer.png"),
        (AppTab::Setup, "Setup_1.png"),
        (AppTab::Analysis, "Analysis.png"),
        (AppTab::Strategy, "Strategy.png"),
    ];

    for (tab, filename) in &targets {
        app.active_tab = *tab;
        terminal.draw(|f| renderer.render(f, &app))?;
        buffer_to_png(terminal.backend().buffer(), width, height, &screenshot_dir.join(filename))?;
        println!("  [OK] Rendered {}", filename);
    }

    // 3. Setup_cloud.png
    app.active_tab = AppTab::Setup;
    *app.setup_manager.browser_active.safe_lock() = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_png(terminal.backend().buffer(), width, height, &screenshot_dir.join("Setup_cloud.png"))?;
    println!("  [OK] Rendered Setup_cloud.png");

    println!("\nALL ENGLISH PNG SCREENSHOTS GENERATED SUCCESSFULLY!");
    Ok(())
}
