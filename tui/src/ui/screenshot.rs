//! Rendering a drawn terminal buffer to a PNG.
//!
//! Shared by the screenshot hotkey and by `tui_tester`, which generates the
//! images in the README. SVG is still how the picture is *described* — laying
//! out a grid of coloured glyphs is a dozen lines of it and a font rasteriser
//! otherwise — but it is now an intermediate held in memory rather than a file
//! anyone has to keep. Two files per screen meant two things to regenerate,
//! two things to review, and one of them GitHub would not render inline.

use std::fs;
use std::path::Path;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// The picture, described. Rasterised by [`buffer_to_png`] and never written
/// out on its own.
fn svg_string(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
    let char_w = 10u32;
    let char_h = 20u32;
    let header_h = 32u32;
    let font_size = 14u32;
    let content_w = (width as u32) * char_w;
    let content_h = (height as u32) * char_h;
    let total_w = content_w + 16;
    let total_h = content_h + header_h + 16;

    let mut svg = String::with_capacity(64 * 1024);
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}" style="background-color: #0b0f19; font-family: 'DejaVu Sans Mono', 'Noto Color Emoji', 'Fira Code', 'JetBrains Mono', 'Cascadia Code', 'Consolas', monospace; font-size: {}px;">"#,
        total_w, total_h, total_w, total_h, font_size
    ));
    svg.push('\n');

    // Outer Card Frame with Shadow & Border
    svg.push_str(&format!(
        r##"  <rect x="4" y="4" width="{}" height="{}" rx="10" fill="#0d1117" stroke="#30363d" stroke-width="1.5"/>"##,
        total_w - 8, total_h - 8
    ));
    svg.push('\n');

    // Window Control Buttons (Red, Yellow, Green)
    svg.push_str(r##"  <circle cx="20" cy="20" r="5.5" fill="#ff5f56"/>"##);
    svg.push('\n');
    svg.push_str(r##"  <circle cx="36" cy="20" r="5.5" fill="#ffbd2e"/>"##);
    svg.push('\n');
    svg.push_str(r##"  <circle cx="52" cy="20" r="5.5" fill="#27c93f"/>"##);
    svg.push('\n');

    // Window Title Text
    svg.push_str(&format!(
        r##"  <text x="{}" y="24" fill="#8b949e" font-size="12px" font-weight="bold" text-anchor="middle">AC Pro Engineer v{} — High-Performance Sim Telemetry Suite</text>"##,
        total_w / 2,
        // Was hardcoded to v0.2.3, so every screenshot in the README claimed
        // a version two releases old.
        ac_core::updater::CURRENT_VERSION
    ));
    svg.push('\n');

    // Divider Line
    svg.push_str(&format!(
        r##"  <line x1="4" y1="{}" x2="{}" y2="{}" stroke="#21262d" stroke-width="1"/>"##,
        header_h,
        total_w - 4,
        header_h
    ));
    svg.push('\n');

    // 1. Draw Cell Backgrounds
    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            let bg_color = match cell.bg {
                ratatui::style::Color::DarkGray => "#21262d",
                ratatui::style::Color::Gray => "#30363d",
                ratatui::style::Color::Blue => "#1f6feb",
                ratatui::style::Color::Red => "#da3633",
                ratatui::style::Color::Green => "#238636",
                ratatui::style::Color::Yellow => "#9e6a03",
                ratatui::style::Color::Cyan => "#1b7c83",
                ratatui::style::Color::Magenta => "#8957e5",
                _ => continue,
            };

            let px = 8 + (x as u32) * char_w;
            let py = header_h + (y as u32) * char_h;
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
                ratatui::style::Color::Red => "#ff7b72",
                ratatui::style::Color::Green => "#7ee787",
                ratatui::style::Color::Yellow => "#f2cc60",
                ratatui::style::Color::Blue => "#79c0ff",
                ratatui::style::Color::Magenta => "#d2a8ff",
                ratatui::style::Color::Cyan => "#56d4dd",
                ratatui::style::Color::Gray => "#8b949e",
                ratatui::style::Color::DarkGray => "#484f58",
                ratatui::style::Color::White => "#f0f6fc",
                _ => "#c9d1d9",
            };

            let px = 8 + (x as u32) * char_w + 1;
            let py = header_h + (y as u32) * char_h + 15;

            let escaped = escape_xml(symbol);
            svg.push_str(&format!(
                r#"  <text x="{}" y="{}" fill="{}">{}</text>"#,
                px, py, fg_color, escaped
            ));
            svg.push('\n');
        }
    }

    svg.push_str("</svg>\n");
    svg
}

/// Render `buffer` to a PNG file at `output_path`.
///
/// The only screenshot format this project produces. The SVG above describes
/// the frame; this rasterises it, because a README on GitHub wants a bitmap
/// and because "here is a picture of the program" should not require anyone to
/// install a converter.
///
/// `scale` is a multiplier on the SVG's own size — 2.0 gives a crisp image on
/// a high-DPI screen at half the on-screen width.
pub fn buffer_to_png(
    buffer: &ratatui::buffer::Buffer,
    width: u16,
    height: u16,
    output_path: &Path,
    scale: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    use resvg::tiny_skia;
    use resvg::usvg;

    let svg = svg_string(buffer, width, height);

    // System fonts, because the monospace stack above has to resolve to
    // something real. Without this every glyph silently renders as nothing and
    // the PNG comes out as an empty dark rectangle.
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    let options = usvg::Options {
        fontdb: std::sync::Arc::new(fontdb),
        ..usvg::Options::default()
    };

    let tree = usvg::Tree::from_str(&svg, &options)?;
    let size = tree.size().to_int_size();
    let target = size
        .scale_by(scale)
        .ok_or("the screenshot scaled to nothing")?;

    let mut pixmap = tiny_skia::Pixmap::new(target.width(), target.height())
        .ok_or("could not allocate the screenshot")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    pixmap.save_png(output_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create tmp");
        dir
    }

    #[test]
    fn renders_the_buffer_contents_into_the_drawing() {
        let area = Rect::new(0, 0, 20, 3);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(0, 0, "ENGINE START", ratatui::style::Style::default());

        let svg = svg_string(&buffer, area.width, area.height);
        assert!(svg.starts_with("<svg"), "starts with an svg root element");
        assert!(svg.trim_end().ends_with("</svg>"), "and closes it");
        // The glyphs are emitted one <text> element per cell, so the string
        // is present character by character rather than as a whole word.
        for ch in "ENGINE".chars() {
            assert!(
                svg.contains(&format!(">{}</text>", ch)),
                "glyph {ch} is in the output"
            );
        }
    }

    /// A terminal shows plenty of characters that are not valid raw XML, and
    /// an ampersand alone would make the intermediate unparseable — which
    /// now means the PNG fails to render rather than a file failing to open.
    #[test]
    fn escapes_characters_that_would_break_the_xml() {
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml(r#"say "hi""#), "say &quot;hi&quot;");

        let area = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(0, 0, "a<b&c", ratatui::style::Style::default());

        let svg = svg_string(&buffer, area.width, area.height);
        assert!(svg.contains("&lt;"), "the < is escaped");
        assert!(svg.contains("&amp;"), "and so is the &");
    }

    /// The one screenshot format there is, produced end to end: an empty
    /// buffer still has to reach a file, because that is the path the hotkey
    /// takes on a screen that happens to be blank.
    #[test]
    fn writes_a_png_even_for_an_empty_buffer() {
        let area = Rect::new(0, 0, 4, 2);
        let buffer = Buffer::empty(area);

        let path = scratch_dir("shot_empty").join("out.png");
        buffer_to_png(&buffer, area.width, area.height, &path, 1.0).expect("write png");

        let bytes = fs::read(&path).expect("read png");
        assert_eq!(
            &bytes[..8],
            b"\x89PNG\r\n\x1a\n",
            "and it is a PNG, not whatever the encoder felt like"
        );
    }
}
