use ratatui::{prelude::*, widgets::*};
use crate::AppState;
use crate::ui::localization::tr;

pub fn render(f: &mut Frame, area: Rect, app: &AppState) {
    let text = Paragraph::new(tr("view_anal", &app.config.language))
        .block(Block::default().title(tr("tab_anal", &app.config.language)).borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(text, area);
}