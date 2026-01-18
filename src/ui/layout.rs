use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn split_main(area: Rect, show_slash: bool) -> (Rect, Rect, Rect, Option<Rect>) {
    if show_slash {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ]
                .as_ref(),
            )
            .split(area);
        return (chunks[0], chunks[1], chunks[2], Some(chunks[3]));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(area);
    (chunks[0], chunks[1], chunks[2], None)
}

pub fn split_panes(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(area);
    (chunks[0], chunks[1])
}
