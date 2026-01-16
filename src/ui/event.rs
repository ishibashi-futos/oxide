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

pub fn is_enter_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Enter
}

pub fn is_enter_dir_event(key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    if key.code == KeyCode::Char(']') {
        return true;
    }
    key.code == KeyCode::Right
        && (key.modifiers.is_empty()
            || key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::SUPER))
}

pub fn is_parent_event(key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    if key.code == KeyCode::Char('[') {
        return true;
    }
    key.code == KeyCode::Left
        && (key.modifiers.is_empty()
            || key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::SUPER))
}

pub fn is_toggle_hidden_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
        && key.code == KeyCode::Char('h')
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_enter_event_requires_press() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert!(is_enter_event(key));
    }

    #[test]
    fn is_enter_dir_event_requires_alt_right() {
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::ALT);
        assert!(is_enter_dir_event(key));
    }

    #[test]
    fn is_enter_dir_event_allows_super_right() {
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::SUPER);
        assert!(is_enter_dir_event(key));
    }

    #[test]
    fn is_enter_dir_event_allows_plain_right() {
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert!(is_enter_dir_event(key));
    }

    #[test]
    fn is_enter_dir_event_allows_right_bracket() {
        let key = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE);
        assert!(is_enter_dir_event(key));
    }

    #[test]
    fn is_parent_event_requires_alt_left() {
        let key = KeyEvent::new(KeyCode::Left, KeyModifiers::ALT);
        assert!(is_parent_event(key));
    }

    #[test]
    fn is_parent_event_allows_super_left() {
        let key = KeyEvent::new(KeyCode::Left, KeyModifiers::SUPER);
        assert!(is_parent_event(key));
    }

    #[test]
    fn is_parent_event_allows_plain_left() {
        let key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
        assert!(is_parent_event(key));
    }

    #[test]
    fn is_parent_event_allows_left_bracket() {
        let key = KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE);
        assert!(is_parent_event(key));
    }

    #[test]
    fn is_toggle_hidden_event_requires_ctrl_h() {
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL);
        assert!(is_toggle_hidden_event(key));
    }

    #[test]
    fn is_toggle_hidden_event_rejects_plain_h() {
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        assert!(!is_toggle_hidden_event(key));
    }
}
