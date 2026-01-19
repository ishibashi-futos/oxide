use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

pub enum PreviewPaneState<'a> {
    Empty,
    Loading,
    Ready {
        lines: &'a [String],
        reason: Option<String>,
        truncated: bool,
    },
    Failed {
        reason: String,
    },
}

pub fn render_preview_pane(frame: &mut Frame<'_>, area: Rect, state: PreviewPaneState<'_>) {
    let block = Block::default().borders(Borders::ALL).title("preview");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let text = Paragraph::new(build_preview_text(state));
    frame.render_widget(text, inner);
}

fn build_preview_text(state: PreviewPaneState<'_>) -> String {
    match state {
        PreviewPaneState::Empty => "preview: empty".to_string(),
        PreviewPaneState::Loading => "preview: loading...".to_string(),
        PreviewPaneState::Ready {
            lines,
            reason,
            truncated,
        } => {
            let mut text = lines.join("\n");
            if let Some(reason) = reason {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&reason);
            }
            if truncated {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push('…');
            }
            text
        }
        PreviewPaneState::Failed { reason } => format!("preview failed: {reason}"),
    }
}
