use crate::AppState;
use ratatui::{layout::*, prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(16),
            Constraint::Percentage(25),
        ])
        .split(area);

    let middle = popup_layout[1];
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Min(50),
            Constraint::Percentage(25),
        ])
        .split(middle);

    let popup_area = horizontal[1];

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " OVERLAY CONTROL CENTER [ESC to close] ",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::White),
        ));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let is_active = app.overlay_manager.is_active;
    let is_unlocked = app.overlay_manager.is_unlocked;

    let items = [
        format!(
            "[{}] 1. Master Overlay Visibility",
            if is_active { "X" } else { " " }
        ),
        format!(
            "[{}] 2. EDIT MODE (Unlock to Move/Resize/Switch Tabs)",
            if is_unlocked { "X" } else { " " }
        ),
    ];

    let mut list_items = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let style = if i == app.overlay_menu_selection {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(Color::White)
        };
        list_items.push(ListItem::new(item.clone()).style(style));
    }

    let list = List::new(list_items).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(inner);

    f.render_widget(list, chunks[0]);

    let instructions = Paragraph::new("\nUse UP/DOWN arrows to select.\nPress ENTER to toggle.\n\nWhen Edit Mode is ON, you can drag the overlay with your mouse and click the top tabs!")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(instructions, chunks[1]);
}
