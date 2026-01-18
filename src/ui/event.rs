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

pub fn is_slash_activate_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Char('/') && key.modifiers.is_empty()
}

pub fn is_slash_cancel_event(key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    match key.code {
        KeyCode::Esc => true,
        KeyCode::Char('c') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

pub fn is_slash_history_prev_event(key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    match key.code {
        KeyCode::Up => true,
        KeyCode::Char('p') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

pub fn is_slash_history_next_event(key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    match key.code {
        KeyCode::Down => true,
        KeyCode::Char('n') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

pub fn is_slash_complete_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Tab
}

pub fn slash_input_char(key: KeyEvent) -> Option<char> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    let KeyCode::Char(ch) = key.code else {
        return None;
    };
    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
        return Some(ch);
    }
    None
}

pub fn search_char(key: KeyEvent) -> Option<char> {
    if key.kind != KeyEventKind::Press {
        return None;
    }
    let KeyCode::Char(ch) = key.code else {
        return None;
    };
    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
        return Some(ch);
    }
    None
}

pub fn is_search_reset_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Esc
}

pub fn is_search_backspace_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Backspace
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

    #[test]
    fn is_slash_activate_event_accepts_plain_slash() {
        let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        assert!(is_slash_activate_event(key));
    }

    #[test]
    fn is_slash_cancel_event_accepts_escape() {
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(is_slash_cancel_event(key));
    }

    #[test]
    fn is_slash_cancel_event_accepts_ctrl_c() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(is_slash_cancel_event(key));
    }

    #[test]
    fn slash_input_char_accepts_plain_char() {
        let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(slash_input_char(key), Some('p'));
    }

    #[test]
    fn slash_input_char_rejects_ctrl_char() {
        let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(slash_input_char(key), None);
    }

    #[test]
    fn is_slash_history_prev_accepts_up() {
        let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert!(is_slash_history_prev_event(key));
    }

    #[test]
    fn is_slash_history_prev_accepts_ctrl_p() {
        let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert!(is_slash_history_prev_event(key));
    }

    #[test]
    fn is_slash_history_next_accepts_down() {
        let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert!(is_slash_history_next_event(key));
    }

    #[test]
    fn is_slash_history_next_accepts_ctrl_n() {
        let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        assert!(is_slash_history_next_event(key));
    }

    #[test]
    fn is_slash_complete_event_accepts_tab() {
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert!(is_slash_complete_event(key));
    }

    #[test]
    fn search_char_accepts_plain_char() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(search_char(key), Some('a'));
    }

    #[test]
    fn search_char_rejects_ctrl_char() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(search_char(key), None);
    }

    #[test]
    fn search_char_rejects_non_char() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(search_char(key), None);
    }

    #[test]
    fn is_search_reset_event_accepts_escape() {
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(is_search_reset_event(key));
    }

    #[test]
    fn is_search_backspace_event_accepts_backspace() {
        let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        assert!(is_search_backspace_event(key));
    }
}
