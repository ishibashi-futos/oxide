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

pub fn split_panes(area: Rect, preview_ratio: Option<u16>) -> (Rect, Rect, Option<Rect>) {
    if let Some(preview_ratio) = preview_ratio {
        let preview_ratio = preview_ratio.clamp(30, 40);
        let parent_ratio = 25;
        let current_ratio = 100u16.saturating_sub(parent_ratio + preview_ratio);
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(parent_ratio),
                    Constraint::Percentage(current_ratio),
                    Constraint::Percentage(preview_ratio),
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

        let (_, _, preview) = split_panes(area, Some(35));

        assert!(preview.is_some());
    }

    #[test]
    fn split_panes_hides_preview_when_not_visible() {
        let area = Rect::new(0, 0, 100, 10);

        let (_, _, preview) = split_panes(area, None);

        assert!(preview.is_none());
    }

    #[test]
    fn split_panes_clamps_preview_ratio_low() {
        let area = Rect::new(0, 0, 100, 10);

        let (_, _, preview) = split_panes(area, Some(10));

        assert_eq!(preview.unwrap().width, 30);
    }

    #[test]
    fn split_panes_clamps_preview_ratio_high() {
        let area = Rect::new(0, 0, 100, 10);

        let (_, _, preview) = split_panes(area, Some(50));

        assert_eq!(preview.unwrap().width, 40);
    }
}
