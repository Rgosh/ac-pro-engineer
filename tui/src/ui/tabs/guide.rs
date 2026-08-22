use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" AC PRO ENGINEER: GRAND PRIX ENGINEERING HANDBOOK ")
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(Color::White));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(inner);

    let current_chapter = app.ui_state.guide_list_state.selected().unwrap_or(0);

    render_toc(f, layout[0], &app.ui_state.guide_list_state);
    render_content(f, layout[1], current_chapter);
}

fn render_toc(f: &mut Frame<'_>, area: Rect, state: &ListState) {
    let block = Block::default().borders(Borders::RIGHT).title(" SECTIONS ");

    // The list comes from the same place the chapters do, so a chapter added
    // to the core appears here without this file being touched — and the
    // numbering can never disagree with the content it points at.
    let items: Vec<ListItem<'_>> = ac_core::guide::CHAPTERS
        .iter()
        .map(|chapter| {
            ListItem::new(Span::styled(
                chapter.title,
                Style::default().fg(Color::White),
            ))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state_clone = state.clone();
    f.render_stateful_widget(list, area, &mut state_clone);
}

fn render_content(f: &mut Frame<'_>, area: Rect, chapter: usize) {
    // **The words come from `ac_core::guide` and the colours stay here.** The
    // chapters used to be written out in this file, which made the terminal
    // their owner — and a second front end drawing the same handbook would
    // have had to keep a copy, with nothing to notice when the two drifted.
    // A copy of a paragraph does not fail a test when it goes stale; it just
    // tells two drivers different things.
    let text: Vec<Line<'_>> = ac_core::guide::CHAPTERS
        .get(chapter)
        .map(|chapter| chapter.lines.iter().map(style).collect())
        .unwrap_or_default();

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" CONTENT "))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

/// One line of the handbook, in the terminal's colours.
///
/// The kinds are not styles — `Secret` is *the line the chapter exists for*,
/// not "yellow italic" — so this is where the terminal decides what its own
/// emphasis looks like, and another front end decides differently without
/// touching the writing.
fn style(line: &ac_core::guide::Line) -> Line<'static> {
    use ac_core::guide::Line as Source;
    match line {
        Source::H1(text) => Line::from(Span::styled(
            *text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Source::H2(text) => Line::from(Span::styled(
            *text,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )),
        Source::P(text) => Line::from(Span::styled(*text, Style::default().fg(Color::Gray))),
        Source::Art(text) => Line::from(Span::styled(*text, Style::default().fg(Color::Green))),
        Source::BadArt(text) => Line::from(Span::styled(*text, Style::default().fg(Color::Red))),
        Source::Secret(text) => Line::from(Span::styled(
            format!("   [SECRET]: {text}"),
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::ITALIC),
        )),
        Source::Warn(text) => Line::from(Span::styled(
            format!("   [!] {text}"),
            Style::default().fg(Color::LightRed),
        )),
        Source::Crit(text) => Line::from(Span::styled(
            format!("   [CRITICAL]: {text}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Source::Fix(text) => Line::from(Span::styled(
            format!("   [FIX]: {text}"),
            Style::default().fg(Color::LightGreen),
        )),
        Source::Math(text) => Line::from(Span::styled(
            format!("   [MATH]: {text}"),
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::ITALIC),
        )),
        Source::Br => Line::from(""),
    }
}
