use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct TabsState {
    tabs: Vec<PathBuf>,
    active: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabSummary {
    pub(crate) number: usize,
    pub(crate) path: PathBuf,
    pub(crate) active: bool,
}

impl TabsState {
    pub(crate) fn new(current_dir: PathBuf) -> Self {
        Self {
            tabs: vec![current_dir],
            active: 0,
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) fn active_number(&self) -> usize {
        self.active + 1
    }

    pub(crate) fn store_active(&mut self, current_dir: &PathBuf) {
        if let Some(slot) = self.tabs.get_mut(self.active) {
            *slot = current_dir.clone();
        }
    }

    pub(crate) fn push_new(&mut self, current_dir: &PathBuf) {
        self.store_active(current_dir);
        self.tabs.push(current_dir.clone());
        self.active = self.tabs.len().saturating_sub(1);
    }

    pub(crate) fn next_index(&self) -> Option<usize> {
        if self.tabs.len() <= 1 {
            return None;
        }
        Some((self.active + 1) % self.tabs.len())
    }

    pub(crate) fn prev_index(&self) -> Option<usize> {
        if self.tabs.len() <= 1 {
            return None;
        }
        Some(if self.active == 0 {
            self.tabs.len().saturating_sub(1)
        } else {
            self.active - 1
        })
    }

    pub(crate) fn switch_to(&mut self, index: usize, current_dir: &PathBuf) -> Option<PathBuf> {
        if index >= self.tabs.len() {
            return None;
        }
        self.store_active(current_dir);
        self.active = index;
        Some(self.tabs[index].clone())
    }

    pub(crate) fn summaries(&self) -> Vec<TabSummary> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, path)| TabSummary {
                number: index + 1,
                path: path.clone(),
                active: index == self.active,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_new_adds_tab_and_sets_active() {
        let dir_one = PathBuf::from("/one");
        let dir_two = PathBuf::from("/two");
        let mut tabs = TabsState::new(dir_one);

        tabs.push_new(&dir_two);

        assert_eq!(tabs.count(), 2);
        assert_eq!(tabs.active_number(), 2);
        assert_eq!(tabs.tabs[0], dir_two);
        assert_eq!(tabs.tabs[1], dir_two);
    }

    #[test]
    fn switch_to_stores_active_and_returns_target() {
        let dir_one = PathBuf::from("/one");
        let dir_two = PathBuf::from("/two");
        let dir_three = PathBuf::from("/three");
        let mut tabs = TabsState {
            tabs: vec![dir_one, dir_two.clone()],
            active: 0,
        };

        let next = tabs.switch_to(1, &dir_three);

        assert_eq!(next, Some(dir_two));
        assert_eq!(tabs.active_number(), 2);
        assert_eq!(tabs.tabs[0], dir_three);
    }

    #[test]
    fn summaries_marks_active_tab() {
        let dir_one = PathBuf::from("/one");
        let dir_two = PathBuf::from("/two");
        let tabs = TabsState {
            tabs: vec![dir_one.clone(), dir_two.clone()],
            active: 1,
        };

        let summaries = tabs.summaries();

        assert_eq!(
            summaries,
            vec![
                TabSummary {
                    number: 1,
                    path: dir_one,
                    active: false,
                },
                TabSummary {
                    number: 2,
                    path: dir_two,
                    active: true,
                },
            ]
        );
    }
}
