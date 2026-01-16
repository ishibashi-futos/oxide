use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub fn is_quit_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
        && key.code == KeyCode::Char('q')
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

pub fn is_cursor_up_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Up
}

pub fn is_cursor_down_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Down
}
