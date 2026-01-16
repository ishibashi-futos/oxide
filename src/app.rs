use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct App {
    pub current_dir: PathBuf,
    pub entries: Vec<String>,
    pub cursor: Option<usize>,
}

impl App {
    pub fn new(current_dir: PathBuf, entries: Vec<String>, cursor: Option<usize>) -> Self {
        Self {
            current_dir,
            entries,
            cursor,
        }
    }

    pub fn move_cursor_up(&mut self) {
        let Some(cursor) = self.cursor else { return };
        if cursor == 0 {
            return;
        }
        self.cursor = Some(cursor - 1);
    }

    pub fn move_cursor_down(&mut self) {
        let Some(cursor) = self.cursor else { return };
        if cursor + 1 >= self.entries.len() {
            return;
        }
        self.cursor = Some(cursor + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_cursor_up_stops_at_top() {
        let mut app = App::new(
            PathBuf::from("."),
            vec!["a.txt".into(), "b.txt".into()],
            Some(0),
        );

        app.move_cursor_up();

        assert_eq!(app.cursor, Some(0));
    }

    #[test]
    fn move_cursor_up_moves_one_step() {
        let mut app = App::new(
            PathBuf::from("."),
            vec!["a.txt".into(), "b.txt".into()],
            Some(1),
        );

        app.move_cursor_up();

        assert_eq!(app.cursor, Some(0));
    }

    #[test]
    fn move_cursor_down_stops_at_bottom() {
        let mut app = App::new(
            PathBuf::from("."),
            vec!["a.txt".into(), "b.txt".into()],
            Some(1),
        );

        app.move_cursor_down();

        assert_eq!(app.cursor, Some(1));
    }

    #[test]
    fn move_cursor_down_moves_one_step() {
        let mut app = App::new(
            PathBuf::from("."),
            vec!["a.txt".into(), "b.txt".into()],
            Some(0),
        );

        app.move_cursor_down();

        assert_eq!(app.cursor, Some(1));
    }
}
