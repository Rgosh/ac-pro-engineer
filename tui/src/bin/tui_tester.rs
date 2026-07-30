use ac_core::config::Language;
use ac_core::overlay::OverlayMode;
use ac_tui::ui::UIRenderer;
use ac_tui::{AppStage, AppState, AppTab, SafeLock};
use image::{ImageBuffer, Rgb};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;
use std::path::Path;

fn get_ascii_glyph(c: char) -> [u8; 12] {
    match c {
        'A' | 'a' => [0x18, 0x24, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00, 0x00, 0x00, 0x00],
        'B' | 'b' => [0x7C, 0x42, 0x42, 0x7C, 0x42, 0x42, 0x42, 0x7C, 0x00, 0x00, 0x00, 0x00],
        'C' | 'c' => [0x3C, 0x42, 0x40, 0x40, 0x40, 0x40, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        'D' | 'd' => [0x78, 0x44, 0x42, 0x42, 0x42, 0x42, 0x44, 0x78, 0x00, 0x00, 0x00, 0x00],
        'E' | 'e' => [0x7E, 0x40, 0x40, 0x7C, 0x40, 0x40, 0x40, 0x7E, 0x00, 0x00, 0x00, 0x00],
        'F' | 'f' => [0x7E, 0x40, 0x40, 0x7C, 0x40, 0x40, 0x40, 0x40, 0x00, 0x00, 0x00, 0x00],
        'G' | 'g' => [0x3C, 0x42, 0x40, 0x4E, 0x42, 0x42, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        'H' | 'h' => [0x42, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x42, 0x00, 0x00, 0x00, 0x00],
        'I' | 'i' => [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00, 0x00, 0x00, 0x00],
        'J' | 'j' => [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x4C, 0x4C, 0x38, 0x00, 0x00, 0x00, 0x00],
        'K' | 'k' => [0x44, 0x48, 0x50, 0x60, 0x50, 0x48, 0x44, 0x42, 0x00, 0x00, 0x00, 0x00],
        'L' | 'l' => [0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x7E, 0x00, 0x00, 0x00, 0x00],
        'M' | 'm' => [0x42, 0x66, 0x5A, 0x42, 0x42, 0x42, 0x42, 0x42, 0x00, 0x00, 0x00, 0x00],
        'N' | 'n' => [0x42, 0x62, 0x52, 0x4A, 0x46, 0x42, 0x42, 0x42, 0x00, 0x00, 0x00, 0x00],
        'O' | 'o' => [0x3C, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        'P' | 'p' => [0x7C, 0x42, 0x42, 0x7C, 0x40, 0x40, 0x40, 0x40, 0x00, 0x00, 0x00, 0x00],
        'Q' | 'q' => [0x3C, 0x42, 0x42, 0x42, 0x4A, 0x44, 0x42, 0x3D, 0x00, 0x00, 0x00, 0x00],
        'R' | 'r' => [0x7C, 0x42, 0x42, 0x7C, 0x50, 0x48, 0x44, 0x42, 0x00, 0x00, 0x00, 0x00],
        'S' | 's' => [0x3C, 0x42, 0x40, 0x3C, 0x02, 0x02, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        'T' | 't' => [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00],
        'U' | 'u' => [0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        'V' | 'v' => [0x42, 0x42, 0x42, 0x42, 0x42, 0x24, 0x24, 0x18, 0x00, 0x00, 0x00, 0x00],
        'W' | 'w' => [0x42, 0x42, 0x42, 0x42, 0x42, 0x5A, 0x66, 0x42, 0x00, 0x00, 0x00, 0x00],
        'X' | 'x' => [0x42, 0x24, 0x18, 0x18, 0x18, 0x24, 0x42, 0x42, 0x00, 0x00, 0x00, 0x00],
        'Y' | 'y' => [0x42, 0x42, 0x24, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00],
        'Z' | 'z' => [0x7E, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x7E, 0x00, 0x00, 0x00, 0x00],
        '0' => [0x3C, 0x46, 0x4A, 0x52, 0x62, 0x42, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        '1' => [0x18, 0x28, 0x08, 0x08, 0x08, 0x08, 0x08, 0x3E, 0x00, 0x00, 0x00, 0x00],
        '2' => [0x3C, 0x42, 0x02, 0x04, 0x18, 0x20, 0x40, 0x7E, 0x00, 0x00, 0x00, 0x00],
        '3' => [0x3C, 0x42, 0x02, 0x1C, 0x02, 0x02, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        '4' => [0x08, 0x18, 0x28, 0x48, 0x7E, 0x08, 0x08, 0x08, 0x00, 0x00, 0x00, 0x00],
        '5' => [0x7E, 0x40, 0x7C, 0x02, 0x02, 0x02, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        '6' => [0x1C, 0x20, 0x40, 0x7C, 0x42, 0x42, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        '7' => [0x7E, 0x02, 0x04, 0x08, 0x10, 0x10, 0x10, 0x10, 0x00, 0x00, 0x00, 0x00],
        '8' => [0x3C, 0x42, 0x42, 0x3C, 0x42, 0x42, 0x42, 0x3C, 0x00, 0x00, 0x00, 0x00],
        '9' => [0x3C, 0x42, 0x42, 0x3E, 0x02, 0x02, 0x04, 0x38, 0x00, 0x00, 0x00, 0x00],
        ':' => [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x08, 0x10, 0x00, 0x00, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '=' => [0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '%' => [0x62, 0x64, 0x08, 0x10, 0x26, 0x46, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00],
        ')' => [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00],
        '[' => [0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00],
        ']' => [0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00],
        '/' => [0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '\\' => [0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '|' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00],
        '!' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00],
        '?' => [0x3C, 0x42, 0x04, 0x08, 0x10, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00],
        '#' => [0x24, 0x24, 0x7E, 0x24, 0x7E, 0x24, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00],
        '*' => [0x00, 0x24, 0x18, 0x7E, 0x18, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '@' => [0x3C, 0x42, 0x5A, 0x5A, 0x5C, 0x40, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00],
        _ => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    }
}

fn buffer_to_png(buffer: &ratatui::buffer::Buffer, width: u16, height: u16, output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let char_w = 9u32;
    let char_h = 18u32;
    let img_w = (width as u32) * char_w;
    let img_h = (height as u32) * char_h;

    let mut imgbuf: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_pixel(img_w, img_h, Rgb([15u8, 20u8, 25u8]));

    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            let symbol = cell.symbol();

            let bg_color = match cell.bg {
                ratatui::style::Color::DarkGray => Rgb([40u8, 45u8, 55u8]),
                ratatui::style::Color::Gray => Rgb([60u8, 65u8, 75u8]),
                ratatui::style::Color::Blue => Rgb([30u8, 58u8, 138u8]),
                _ => Rgb([15u8, 20u8, 25u8]),
            };

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

            // 1. Draw cell background
            for px in start_x..(start_x + char_w) {
                for py in start_y..(start_y + char_h) {
                    if px < img_w && py < img_h {
                        imgbuf.put_pixel(px, py, bg_color);
                    }
                }
            }

            if symbol.is_empty() || symbol == " " {
                continue;
            }

            let first_char = symbol.chars().next().unwrap_or(' ');

            // 2. Box drawing characters & block characters
            match first_char {
                '─' | '═' => {
                    let mid_y = start_y + char_h / 2;
                    for px in start_x..(start_x + char_w) {
                        if px < img_w && mid_y < img_h {
                            imgbuf.put_pixel(px, mid_y, fg_color);
                            imgbuf.put_pixel(px, mid_y + 1, fg_color);
                        }
                    }
                }
                '│' | '║' => {
                    let mid_x = start_x + char_w / 2;
                    for py in start_y..(start_y + char_h) {
                        if mid_x < img_w && py < img_h {
                            imgbuf.put_pixel(mid_x, py, fg_color);
                            imgbuf.put_pixel(mid_x + 1, py, fg_color);
                        }
                    }
                }
                '┌' | '╔' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for px in mid_x..(start_x + char_w) {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                    for py in mid_y..(start_y + char_h) {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                }
                '┐' | '╗' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for px in start_x..=mid_x {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                    for py in mid_y..(start_y + char_h) {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                }
                '└' | '╚' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for px in mid_x..(start_x + char_w) {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                    for py in start_y..=mid_y {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                }
                '┘' | '╝' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for px in start_x..=mid_x {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                    for py in start_y..=mid_y {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                }
                '├' | '╠' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for py in start_y..(start_y + char_h) {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                    for px in mid_x..(start_x + char_w) {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                }
                '┤' | '╣' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for py in start_y..(start_y + char_h) {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                    for px in start_x..=mid_x {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                }
                '┬' | '╦' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for px in start_x..(start_x + char_w) {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                    for py in mid_y..(start_y + char_h) {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                }
                '┴' | '╩' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for px in start_x..(start_x + char_w) {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                    for py in start_y..=mid_y {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                }
                '┼' | '╬' => {
                    let mid_x = start_x + char_w / 2;
                    let mid_y = start_y + char_h / 2;
                    for px in start_x..(start_x + char_w) {
                        imgbuf.put_pixel(px, mid_y, fg_color);
                    }
                    for py in start_y..(start_y + char_h) {
                        imgbuf.put_pixel(mid_x, py, fg_color);
                    }
                }
                '█' => {
                    for px in start_x..(start_x + char_w) {
                        for py in start_y..(start_y + char_h) {
                            if px < img_w && py < img_h {
                                imgbuf.put_pixel(px, py, fg_color);
                            }
                        }
                    }
                }
                _ => {
                    // Standard ASCII text glyph
                    let glyph = get_ascii_glyph(first_char);
                    for (row_idx, &row_byte) in glyph.iter().enumerate() {
                        for col_idx in 0..8 {
                            if (row_byte & (1 << (7 - col_idx))) != 0 {
                                let px = start_x + (col_idx as u32) + 1;
                                let py = start_y + (row_idx as u32) + 3;
                                if px < img_w && py < img_h {
                                    imgbuf.put_pixel(px, py, fg_color);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    imgbuf.save(output_path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting English PNG Screenshot Generator (Text Glyph Rasterizer)...");

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

    println!("\nALL ENGLISH PNG SCREENSHOTS GENERATED WITH CRISP TEXT GLYPHS!");
    Ok(())
}
