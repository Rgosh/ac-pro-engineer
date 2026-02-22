use ratatui::{prelude::*, widgets::*};
use std::error::Error;
use std::fs;

#[derive(Debug, Clone)]
pub struct FileMenu {
    pub active: bool,
    pub files: Vec<String>,
    pub state: ListState,
}

impl FileMenu {
    pub fn new() -> Self {
        Self {
            active: false,
            files: Vec::new(),
            state: ListState::default(),
        }
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
        if self.active {
            self.refresh_files();
            if !self.files.is_empty() {
                self.state.select(Some(0));
            }
        }
    }

    pub fn refresh_files(&mut self) -> Result<(), Box<dyn Error>> {
        self.files.clear();
        let dir = "saved_laps";
        if fs::metadata(dir).is_err() {
            fs::create_dir(dir)?;
        }

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_file()
                    && let Some(name) = entry.file_name().to_str()
                    && name.ends_with(".json")
                {
                    self.files.push(name.to_string());
                }
            }
        }
        self.files.sort();
        self.files.reverse();
        Ok(())
    }

    pub fn next(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.files.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.files.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn get_selected(&self) -> Option<String> {
        self.state
            .selected()
            .and_then(|i| self.files.get(i).cloned())
    }
}

pub fn render(f: &mut Frame<'_>, area: Rect, menu: &mut FileMenu, is_ru: bool) {
    let popup_area = centered_rect(60, 70, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(if is_ru {
            " Загрузить Телеметрию "
        } else {
            " Load Telemetry "
        })
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Cyan));

    if menu.files.is_empty() {
        let text = Paragraph::new(if is_ru {
            "Нет сохраненных файлов.\nПапка 'saved_laps' пуста."
        } else {
            "No saved files found.\nCheck 'saved_laps' folder."
        })
        .alignment(Alignment::Center)
        .block(block)
        .style(Style::default().fg(Color::DarkGray));
        f.render_widget(text, popup_area);
    } else {
        let items: Vec<ListItem<'_>> = menu
            .files
            .iter()
            .map(|file| {
                let clean_name = file.replace(".json", "").replace("_", " ");
                ListItem::new(Line::from(vec![
                    Span::styled(" 💾 ", Style::default().fg(Color::Yellow)),
                    Span::styled(clean_name, Style::default().fg(Color::White)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(" ► ");

        f.render_stateful_widget(list, popup_area, &mut menu.state);
    }

    let help_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(popup_area)[1];

    let help_text = if is_ru {
        "ENTER: Загрузить | ESC: Закрыть"
    } else {
        "ENTER: Load | ESC: Close"
    };
    f.render_widget(
        Paragraph::new(help_text)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Black).fg(Color::Gray)),
        help_area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
