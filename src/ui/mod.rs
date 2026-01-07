use ratatui::{prelude::*, widgets::*};
use crate::{AppState, AppTab};
use crate::ui::localization::tr;

pub mod localization;
pub mod widgets;
pub mod tabs;

pub struct UIState {
    pub theme: crate::config::Theme,
    pub layout_mode: LayoutMode,
    pub show_help: bool,
    pub blink_state: bool,
    pub last_blink: std::time::Instant,
    pub settings: tabs::settings::SettingsState,
    pub setup_list_state: ListState, // Добавлено: состояние списка сетапов
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    Horizontal,
    Vertical,
    Auto,
}

impl UIState {
    pub fn new(theme: &crate::config::Theme) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0)); // Выбираем первый элемент по умолчанию

        Self {
            theme: theme.clone(),
            layout_mode: LayoutMode::Auto,
            show_help: false,
            blink_state: false,
            last_blink: std::time::Instant::now(),
            settings: tabs::settings::SettingsState::new(),
            setup_list_state: list_state,
        }
    }
    
    pub fn get_color(&self, color_tuple: &crate::config::ColorTuple) -> Color {
        Color::Rgb(color_tuple.r, color_tuple.g, color_tuple.b)
    }

    pub fn update_blink(&mut self) {
        if self.last_blink.elapsed() >= std::time::Duration::from_millis(500) {
            self.blink_state = !self.blink_state;
            self.last_blink = std::time::Instant::now();
        }
    }
}

pub struct UIRenderer;

impl UIRenderer {
    pub fn new() -> Self {
        Self
    }
    
    pub fn render(&self, f: &mut Frame, app: &AppState) {
        let size = f.size();
        let is_vertical = size.height as f32 > size.width as f32 * 1.5;
        let layout_mode = if app.ui_state.layout_mode == LayoutMode::Auto {
            if is_vertical { LayoutMode::Vertical } else { LayoutMode::Horizontal }
        } else {
            app.ui_state.layout_mode
        };
        
        match layout_mode {
            LayoutMode::Horizontal => self.render_horizontal(f, app),
            LayoutMode::Vertical => self.render_vertical(f, app),
            LayoutMode::Auto => self.render_horizontal(f, app),
        }
    }
    
    fn render_horizontal(&self, f: &mut Frame, app: &AppState) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(f.size());
        
        self.render_header(f, main_layout[0], app);
        
        match app.active_tab {
            AppTab::Dashboard => tabs::dashboard::render_horizontal(f, main_layout[1], app),
            AppTab::Telemetry => tabs::telemetry::render(f, main_layout[1], app),
            AppTab::Engineer => tabs::engineer::render_horizontal(f, main_layout[1], app),
            AppTab::Setup => tabs::setup::render(f, main_layout[1], app), // Обратите внимание: функция принимает mut app если нужно обновлять стейт
            AppTab::Analysis => tabs::analysis::render(f, main_layout[1], app),
            AppTab::Strategy => tabs::strategy::render(f, main_layout[1], app),
            AppTab::Settings => tabs::settings::render(f, main_layout[1], app),
        }
        
        self.render_footer(f, main_layout[2], app);
    }
    
    fn render_vertical(&self, f: &mut Frame, app: &AppState) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(12),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(f.size());
        
        self.render_header(f, main_layout[0], app);
        widgets::render_telemetry_bar_vertical(f, main_layout[1], app);
        
        match app.active_tab {
            AppTab::Dashboard => tabs::dashboard::render_vertical(f, main_layout[2], app),
            AppTab::Telemetry => tabs::telemetry::render(f, main_layout[2], app),
            AppTab::Engineer => tabs::engineer::render_vertical(f, main_layout[2], app),
            AppTab::Setup => tabs::setup::render(f, main_layout[2], app),
            AppTab::Analysis => tabs::analysis::render(f, main_layout[2], app),
            AppTab::Strategy => tabs::strategy::render(f, main_layout[2], app),
            AppTab::Settings => tabs::settings::render(f, main_layout[2], app),
        }
        
        self.render_footer(f, main_layout[3], app);
    }
    
    fn render_header(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let theme = &app.ui_state.theme;
        let lang = &app.config.language;
        
        let tabs = vec![
            format!("🏁 {}", tr("tab_dash", lang)),
            format!("📊 {}", tr("tab_tele", lang)), 
            format!("👨‍🔧 {}", tr("tab_eng", lang)),
            format!("🔧 {}", tr("tab_setup", lang)),
            format!("📈 {}", tr("tab_anal", lang)),
            format!("🎯 {}", tr("tab_strat", lang)),
            format!("⚙️ {}", tr("tab_set", lang)),
        ];
        
        let tab_widget = Tabs::new(tabs)
            .select(match app.active_tab {
                AppTab::Dashboard => 0,
                AppTab::Telemetry => 1,
                AppTab::Engineer => 2,
                AppTab::Setup => 3,
                AppTab::Analysis => 4,
                AppTab::Strategy => 5,
                AppTab::Settings => 6,
            })
            .block(Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(app.ui_state.get_color(&theme.border))))
            .style(Style::default().fg(app.ui_state.get_color(&theme.text)))
            .highlight_style(Style::default()
                .fg(app.ui_state.get_color(&theme.highlight))
                .add_modifier(Modifier::BOLD))
            .divider("│");
        
        f.render_widget(tab_widget, area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect, app: &AppState) {
        let lang = &app.config.language;
        let status = if app.is_connected {
            let blink = if app.ui_state.blink_state { "●" } else { "○" };
            format!("{} {} | {}", blink, tr("footer_connected", lang), tr("footer_keys", lang))
        } else {
             format!("{} | Settings: Enter/Arrows", tr("footer_disconnected", lang))
        };
        
        let status_color = if app.is_connected { Color::Green } else { Color::Red };
        
        let footer = Paragraph::new(status)
            .style(Style::default().fg(status_color))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::TOP));
        
        f.render_widget(footer, area);
    }
}