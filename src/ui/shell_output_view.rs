use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

pub fn render_shell_output_view(frame: &mut Frame<'_>, area: Rect, text: &str) {
    let block = Block::default().borders(Borders::ALL).title("shell output");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let paragraph = Paragraph::new(text.to_string());
    frame.render_widget(paragraph, inner);
}
