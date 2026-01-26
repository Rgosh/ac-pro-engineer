use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let theme = &app.ui_state.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.ui_state.get_color(&theme.border)))
        .title(" OVERLAY MODE ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let p = Paragraph::new("Overlay mode active. Press F10 to exit.")
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.ui_state.get_color(&theme.text)));

    f.render_widget(p, inner);
}
