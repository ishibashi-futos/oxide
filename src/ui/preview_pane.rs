use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

pub fn render_preview_pane(frame: &mut Frame<'_>, area: Rect, status: &str) {
    let block = Block::default().borders(Borders::ALL).title("preview");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let text = Paragraph::new(status);
    frame.render_widget(text, inner);
}
