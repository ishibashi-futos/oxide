use std::path::{Path, PathBuf};

use crate::core::{ColorThemeId, SessionTab};

#[derive(Debug, Clone)]
pub(crate) struct TabsState {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    rotation: ThemeRotation,
    events: Vec<TabsEvent>,
}

#[derive(Debug, Clone)]
pub(crate) struct Tab {
    id: u64,
    path: PathBuf,
    theme_id: ColorThemeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ColorPreference {
    pub(crate) tab_id: u64,
    pub(crate) theme_id: ColorThemeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabSummary {
    pub(crate) number: usize,
    pub(crate) path: PathBuf,
    pub(crate) active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TabsEvent {
    ActivePathChanged { tab_id: u64, path: PathBuf },
    ActiveThemeChanged { tab_id: u64, theme_id: ColorThemeId },
    TabAdded { tab_id: u64, path: PathBuf },
}

#[derive(Debug, Clone)]
struct ThemeRotation {
    order: Vec<ColorThemeId>,
    index: usize,
}

impl TabsState {
    pub(crate) fn new(current_dir: PathBuf, default_theme: Option<ColorThemeId>) -> Self {
        let start = default_theme.unwrap_or(ColorThemeId::GlacierCoast);
        let mut rotation = ThemeRotation::new(start);
        let theme_id = rotation.next();
        Self {
            tabs: vec![Tab {
                id: 1,
                path: current_dir,
                theme_id,
            }],
            active: 0,
            next_id: 2,
            rotation,
            events: Vec::new(),
        }
    }

    pub(crate) fn from_session(tabs: Vec<SessionTab>, default_theme: Option<ColorThemeId>) -> Self {
        let fallback = default_theme.unwrap_or(ColorThemeId::GlacierCoast);
        let mut max_id = 0;
        let restored_tabs = tabs
            .into_iter()
            .map(|tab| {
                let theme_id = ColorThemeId::from_name(&tab.theme_name).unwrap_or(fallback);
                max_id = max_id.max(tab.tab_id);
                Tab {
                    id: tab.tab_id,
                    path: tab.path,
                    theme_id,
                }
            })
            .collect::<Vec<_>>();
        let next_id = max_id.saturating_add(1).max(1);
        Self {
            tabs: restored_tabs,
            active: 0,
            next_id,
            rotation: ThemeRotation::new(fallback),
            events: Vec::new(),
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) fn active_number(&self) -> usize {
        self.active + 1
    }

    pub(crate) fn update_active_path(&mut self, current_dir: &Path) {
        if let Some(slot) = self.tabs.get_mut(self.active) {
            if slot.path == current_dir {
                return;
            }
            slot.path = current_dir.to_path_buf();
            self.events.push(TabsEvent::ActivePathChanged {
                tab_id: slot.id,
                path: slot.path.clone(),
            });
        }
    }

    pub(crate) fn push_new(&mut self, current_dir: &Path) {
        self.update_active_path(current_dir);
        let theme_id = self.rotation.next();
        let tab_id = self.next_id;
        let path = current_dir.to_path_buf();
        self.tabs.push(Tab {
            id: tab_id,
            path: path.clone(),
            theme_id,
        });
        self.next_id = self.next_id.saturating_add(1);
        self.active = self.tabs.len().saturating_sub(1);
        self.events.push(TabsEvent::TabAdded { tab_id, path });
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

    pub(crate) fn switch_to(&mut self, index: usize, current_dir: &Path) -> Option<PathBuf> {
        if index >= self.tabs.len() {
            return None;
        }
        self.update_active_path(current_dir);
        self.active = index;
        Some(self.tabs[index].path.clone())
    }

    pub(crate) fn summaries(&self) -> Vec<TabSummary> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| TabSummary {
                number: index + 1,
                path: tab.path.clone(),
                active: index == self.active,
            })
            .collect()
    }

    pub(crate) fn active_tab_id(&self) -> u64 {
        self.tabs.get(self.active).map(|tab| tab.id).unwrap_or(0)
    }

    pub(crate) fn active_theme_id(&self) -> ColorThemeId {
        self.tabs
            .get(self.active)
            .map(|tab| tab.theme_id)
            .unwrap_or(ColorThemeId::GlacierCoast)
    }

    pub(crate) fn set_active_theme(&mut self, theme_id: ColorThemeId) -> ColorPreference {
        let tab = self.tabs.get_mut(self.active).expect("active tab exists");
        if tab.theme_id != theme_id {
            self.events.push(TabsEvent::ActiveThemeChanged {
                tab_id: tab.id,
                theme_id,
            });
        }
        tab.set_theme(theme_id);
        tab.color_preference()
    }

    pub(crate) fn session_tabs(&self) -> Vec<SessionTab> {
        self.tabs
            .iter()
            .map(|tab| SessionTab {
                tab_id: tab.id,
                path: tab.path.clone(),
                theme_name: tab.theme_id.name().to_string(),
            })
            .collect()
    }

    pub(crate) fn take_events(&mut self) -> Vec<TabsEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Tab {
    pub(crate) fn set_theme(&mut self, theme_id: ColorThemeId) {
        self.theme_id = theme_id;
    }

    #[allow(dead_code)]
    // TODO: タブ一覧やデバッグ表示で参照予定のため保留。
    pub(crate) fn current_theme(&self) -> ColorThemeId {
        self.theme_id
    }

    pub(crate) fn color_preference(&self) -> ColorPreference {
        ColorPreference {
            tab_id: self.id,
            theme_id: self.theme_id,
        }
    }
}

impl ThemeRotation {
    fn new(start: ColorThemeId) -> Self {
        let order = ColorThemeId::all().to_vec();
        let index = order.iter().position(|id| *id == start).unwrap_or(0);
        Self { order, index }
    }

    fn next(&mut self) -> ColorThemeId {
        let theme = self.order[self.index];
        self.index = (self.index + 1) % self.order.len();
        theme
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_new_adds_tab_and_sets_active() {
        let dir_one = PathBuf::from("/one");
        let dir_two = PathBuf::from("/two");
        let mut tabs = TabsState::new(dir_one, None);

        tabs.push_new(&dir_two);

        assert_eq!(tabs.count(), 2);
        assert_eq!(tabs.active_number(), 2);
        assert_eq!(tabs.tabs[0].path, dir_two);
        assert_eq!(tabs.tabs[1].path, dir_two);
    }

    #[test]
    fn switch_to_stores_active_and_returns_target() {
        let dir_one = PathBuf::from("/one");
        let dir_two = PathBuf::from("/two");
        let dir_three = PathBuf::from("/three");
        let mut tabs = TabsState {
            tabs: vec![
                Tab {
                    id: 1,
                    path: dir_one,
                    theme_id: ColorThemeId::GlacierCoast,
                },
                Tab {
                    id: 2,
                    path: dir_two.clone(),
                    theme_id: ColorThemeId::NightHarbor,
                },
            ],
            active: 0,
            next_id: 3,
            rotation: ThemeRotation::new(ColorThemeId::GlacierCoast),
            events: Vec::new(),
        };

        let next = tabs.switch_to(1, &dir_three);

        assert_eq!(next, Some(dir_two));
        assert_eq!(tabs.active_number(), 2);
        assert_eq!(tabs.tabs[0].path, dir_three);
    }

    #[test]
    fn summaries_marks_active_tab() {
        let dir_one = PathBuf::from("/one");
        let dir_two = PathBuf::from("/two");
        let tabs = TabsState {
            tabs: vec![
                Tab {
                    id: 1,
                    path: dir_one.clone(),
                    theme_id: ColorThemeId::GlacierCoast,
                },
                Tab {
                    id: 2,
                    path: dir_two.clone(),
                    theme_id: ColorThemeId::NightHarbor,
                },
            ],
            active: 1,
            next_id: 3,
            rotation: ThemeRotation::new(ColorThemeId::GlacierCoast),
            events: Vec::new(),
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

    #[test]
    fn tab_theme_can_be_set_and_read() {
        let mut tab = Tab {
            id: 1,
            path: PathBuf::from("/one"),
            theme_id: ColorThemeId::GlacierCoast,
        };

        tab.set_theme(ColorThemeId::DeepForest);

        assert_eq!(tab.current_theme(), ColorThemeId::DeepForest);
        assert_eq!(
            tab.color_preference(),
            ColorPreference {
                tab_id: 1,
                theme_id: ColorThemeId::DeepForest
            }
        );
    }
}
