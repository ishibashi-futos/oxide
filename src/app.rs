use std::path::{Path, PathBuf};

use crate::core::{list_entries, Entry};
use crate::error::AppResult;

pub trait EntryOpener {
    fn open(&self, path: &Path) -> AppResult<()>;
}

#[derive(Debug, Clone)]
pub struct App {
    pub current_dir: PathBuf,
    pub entries: Vec<Entry>,
    pub parent_entries: Vec<Entry>,
    pub cursor: Option<usize>,
    pub show_hidden: bool,
}

impl App {
    pub fn new(
        current_dir: PathBuf,
        entries: Vec<Entry>,
        parent_entries: Vec<Entry>,
        cursor: Option<usize>,
        show_hidden: bool,
    ) -> Self {
        Self {
            current_dir,
            entries,
            parent_entries,
            cursor,
            show_hidden,
        }
    }

    pub fn load(current_dir: PathBuf) -> AppResult<Self> {
        let show_hidden = false;
        let entries = list_entries(&current_dir, show_hidden)?;
        let parent_entries = list_parent_entries(&current_dir, show_hidden)?;
        let cursor = if entries.is_empty() { None } else { Some(0) };
        Ok(Self::new(
            current_dir,
            entries,
            parent_entries,
            cursor,
            show_hidden,
        ))
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

    pub fn open_selected(&mut self, opener: &dyn EntryOpener) -> AppResult<()> {
        let Some(selected) = self.selected_entry() else {
            return Ok(());
        };
        let target = self.current_dir.join(&selected.name);
        if selected.is_dir {
            self.current_dir = target;
            self.refresh()?;
            return Ok(());
        }
        opener.open(&target)
    }

    pub fn enter_selected_dir(&mut self) -> AppResult<()> {
        let Some(selected) = self.selected_entry() else {
            return Ok(());
        };
        if !selected.is_dir {
            return Ok(());
        }
        let target = self.current_dir.join(&selected.name);
        self.current_dir = target;
        self.refresh()
    }

    pub fn move_to_parent(&mut self) -> AppResult<()> {
        let Some(parent) = self.current_dir.parent() else {
            return Ok(());
        };
        self.current_dir = parent.to_path_buf();
        self.refresh()
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.cursor.and_then(|index| self.entries.get(index))
    }

    pub fn toggle_hidden(&mut self) -> AppResult<()> {
        let selected_name = self.selected_entry().map(|entry| entry.name.clone());
        let selected_index = self.cursor;
        self.show_hidden = !self.show_hidden;
        self.reload_entries()?;
        self.cursor = resolve_cursor(&self.entries, selected_name.as_deref(), selected_index);
        Ok(())
    }

    fn refresh(&mut self) -> AppResult<()> {
        self.reload_entries()?;
        self.cursor = if self.entries.is_empty() { None } else { Some(0) };
        Ok(())
    }

    fn reload_entries(&mut self) -> AppResult<()> {
        self.entries = list_entries(&self.current_dir, self.show_hidden)?;
        self.parent_entries = list_parent_entries(&self.current_dir, self.show_hidden)?;
        Ok(())
    }
}

fn list_parent_entries(current_dir: &Path, show_hidden: bool) -> AppResult<Vec<Entry>> {
    let Some(parent) = current_dir.parent() else {
        return Ok(Vec::new());
    };
    list_entries(parent, show_hidden)
}

fn resolve_cursor(
    entries: &[Entry],
    selected_name: Option<&str>,
    selected_index: Option<usize>,
) -> Option<usize> {
    if entries.is_empty() {
        return None;
    }
    if let Some(name) = selected_name {
        if let Some(index) = entries.iter().position(|entry| entry.name == name) {
            return Some(index);
        }
        if let Some(index) = selected_index {
            return Some(index.saturating_sub(1).min(entries.len().saturating_sub(1)));
        }
    }
    Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_cursor_up_stops_at_top() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(0),
            false,
        );

        app.move_cursor_up();

        assert_eq!(app.cursor, Some(0));
    }

    #[test]
    fn move_cursor_up_moves_one_step() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(1),
            false,
        );

        app.move_cursor_up();

        assert_eq!(app.cursor, Some(0));
    }

    #[test]
    fn move_cursor_down_stops_at_bottom() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(1),
            false,
        );

        app.move_cursor_down();

        assert_eq!(app.cursor, Some(1));
    }

    #[test]
    fn move_cursor_down_moves_one_step() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(0),
            false,
        );

        app.move_cursor_down();

        assert_eq!(app.cursor, Some(1));
    }

    #[test]
    fn open_selected_enters_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let child_dir = temp_dir.path().join("child");
        std::fs::create_dir(&child_dir).unwrap();

        let entries = vec![Entry {
            name: "child".to_string(),
            is_dir: true,
        }];
        let mut app = App::new(
            temp_dir.path().to_path_buf(),
            entries,
            Vec::new(),
            Some(0),
            false,
        );

        let opener = RecordingOpener::default();
        app.open_selected(&opener).unwrap();

        assert_eq!(app.current_dir, child_dir);
        assert!(opener.opened_paths.borrow().is_empty());
    }

    #[test]
    fn open_selected_uses_opener_for_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("note.txt");
        std::fs::write(&file, "hi").unwrap();

        let entries = vec![Entry {
            name: "note.txt".to_string(),
            is_dir: false,
        }];
        let mut app = App::new(
            temp_dir.path().to_path_buf(),
            entries,
            Vec::new(),
            Some(0),
            false,
        );

        let opener = RecordingOpener::default();
        app.open_selected(&opener).unwrap();

        assert_eq!(
            opener.opened_paths.borrow().as_slice(),
            &[file.clone()]
        );
    }

    #[test]
    fn enter_selected_dir_skips_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("note.txt");
        std::fs::write(&file, "hi").unwrap();

        let entries = vec![Entry {
            name: "note.txt".to_string(),
            is_dir: false,
        }];
        let mut app = App::new(
            temp_dir.path().to_path_buf(),
            entries,
            Vec::new(),
            Some(0),
            false,
        );

        app.enter_selected_dir().unwrap();

        assert_eq!(app.current_dir, temp_dir.path());
    }

    #[test]
    fn move_to_parent_updates_current_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let child_dir = temp_dir.path().join("child");
        std::fs::create_dir(&child_dir).unwrap();

        let mut app = App::load(child_dir).unwrap();

        app.move_to_parent().unwrap();

        assert_eq!(app.current_dir, temp_dir.path());
    }

    #[test]
    fn toggle_hidden_refreshes_entries() {
        let temp_dir = tempfile::tempdir().unwrap();
        let hidden = temp_dir.path().join(".secret");
        std::fs::write(&hidden, "hidden").unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        assert!(app.entries.is_empty());

        app.toggle_hidden().unwrap();

        assert_eq!(
            app.entries,
            vec![Entry {
                name: ".secret".to_string(),
                is_dir: false,
            }]
        );
    }

    #[test]
    fn toggle_hidden_keeps_cursor_on_same_entry() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join(".secret"), "hidden").unwrap();
        std::fs::write(temp_dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(temp_dir.path().join("b.txt"), "b").unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        app.cursor = Some(1);

        app.toggle_hidden().unwrap();

        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("b.txt")
        );
    }

    #[test]
    fn toggle_hidden_moves_to_previous_when_hidden_selected() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join(".secret"), "hidden").unwrap();
        std::fs::write(temp_dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(temp_dir.path().join("b.txt"), "b").unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        app.toggle_hidden().unwrap();
        app.cursor = Some(0);

        app.toggle_hidden().unwrap();

        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("a.txt")
        );
    }

    #[derive(Default)]
    struct RecordingOpener {
        opened_paths: std::cell::RefCell<Vec<PathBuf>>,
    }

    impl EntryOpener for RecordingOpener {
        fn open(&self, path: &Path) -> AppResult<()> {
            self.opened_paths.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }
}
