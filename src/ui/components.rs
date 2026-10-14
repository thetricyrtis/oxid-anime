use ratatui::{
    style::{Color, Style},
    widgets::{Block, BorderType, Borders},
};

pub fn create_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title.to_string())
        .border_style(Style::default().fg(Color::Reset))
}
