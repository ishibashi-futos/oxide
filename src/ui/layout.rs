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

pub fn split_panes(area: Rect, preview_visible: bool) -> (Rect, Rect, Option<Rect>) {
    if preview_visible {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(25),
                    Constraint::Percentage(45),
                    Constraint::Percentage(30),
                ]
                .as_ref(),
            )
            .split(area);
        return (chunks[0], chunks[1], Some(chunks[2]));
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(area);
    (chunks[0], chunks[1], None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_panes_returns_preview_when_visible() {
        let area = Rect::new(0, 0, 100, 10);

        let (_, _, preview) = split_panes(area, true);

        assert!(preview.is_some());
    }

    #[test]
    fn split_panes_hides_preview_when_not_visible() {
        let area = Rect::new(0, 0, 100, 10);

        let (_, _, preview) = split_panes(area, false);

        assert!(preview.is_none());
    }
}
