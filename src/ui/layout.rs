use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn split_main(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(area);
    (chunks[0], chunks[1])
}
