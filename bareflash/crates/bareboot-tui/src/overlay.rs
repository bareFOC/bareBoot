use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandItem {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub shortcut: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPaletteState {
    pub query: String,
    pub selected: usize,
    pub all_items: Vec<CommandItem>,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        let all_items = vec![
            CommandItem { id: "makeflash:run", title: "Katapult Makeflash (Build & Deploy)", category: "Flasher", shortcut: "Space" },
            CommandItem { id: "build:cargo", title: "Run Cargo Build Only", category: "Build", shortcut: "b" },
            CommandItem { id: "flash:direct", title: "Flash Discovered Binary", category: "Flash", shortcut: "f" },
            CommandItem { id: "firmware:pick", title: "Browse Binary File Picker", category: "Flash", shortcut: "o" },
            CommandItem { id: "target:cycle", title: "Cycle Build Target (blink-test, bareboot)", category: "Build", shortcut: "t" },
            CommandItem { id: "deploy:cycle", title: "Cycle Deploy Method (Probe, CAN, Serial)", category: "Flasher", shortcut: "d" },
            CommandItem { id: "probe:list", title: "Detect Hardware Debuggers", category: "Probe", shortcut: "p" },
            CommandItem { id: "probe:reset", title: "Hardware Reset Target MCU", category: "Probe", shortcut: "r" },
            CommandItem { id: "bus:scan", title: "Broadcast Bus Node Scan", category: "Bus", shortcut: "s" },
            CommandItem { id: "bus:pause", title: "Pause/Resume Packet Stream", category: "Console", shortcut: "Space" },
            CommandItem { id: "tab:config", title: "Switch to 1. Target Config", category: "Tabs", shortcut: "1" },
            CommandItem { id: "tab:flasher", title: "Switch to 2. Flasher (Build & Flash)", category: "Tabs", shortcut: "2" },
            CommandItem { id: "tab:console", title: "Switch to 3. Console & Traffic", category: "Tabs", shortcut: "3" },
            CommandItem { id: "theme:next", title: "Cycle Visual Theme", category: "Theme", shortcut: "T" },
        ];
        Self {
            query: String::new(),
            selected: 0,
            all_items,
        }
    }

    pub fn filtered_items(&self) -> Vec<&CommandItem> {
        let q = self.query.to_lowercase();
        self.all_items
            .iter()
            .filter(|item| {
                if q.is_empty() {
                    true
                } else {
                    item.title.to_lowercase().contains(&q)
                        || item.category.to_lowercase().contains(&q)
                        || item.id.to_lowercase().contains(&q)
                }
            })
            .collect()
    }

    pub fn next(&mut self) {
        let count = self.filtered_items().len();
        if count > 0 {
            self.selected = (self.selected + 1) % count;
        }
    }

    pub fn prev(&mut self) {
        let count = self.filtered_items().len();
        if count > 0 {
            self.selected = if self.selected == 0 { count - 1 } else { self.selected - 1 };
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePickerState {
    pub current_dir: PathBuf,
    pub entries: Vec<PathBuf>,
    pub selected: usize,
}

impl FilePickerState {
    pub fn new<P: AsRef<Path>>(dir: P) -> Self {
        let mut state = Self {
            current_dir: dir.as_ref().to_path_buf(),
            entries: Vec::new(),
            selected: 0,
        };
        state.refresh();
        state
    }

    pub fn refresh(&mut self) {
        let mut list = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_dir = path.is_dir();
                let is_firmware = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| matches!(ext, "bin" | "elf" | "hex" | "toml"))
                    .unwrap_or(false);
                if is_dir || is_firmware {
                    list.push(path);
                }
            }
        }
        list.sort();
        self.entries = list;
        if self.selected >= self.entries.len() {
            self.selected = 0;
        }
    }

    pub fn next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.entries.is_empty() {
            self.selected = if self.selected == 0 { self.entries.len() - 1 } else { self.selected - 1 };
        }
    }

    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.entries.get(self.selected)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilePickerState {
    pub names: Vec<String>,
    pub selected: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectModalItem {
    pub id: String,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectModalState {
    pub title: String,
    pub category: String,
    pub items: Vec<SelectModalItem>,
    pub selected: usize,
    pub target_setting: String,
}

impl SelectModalState {
    pub fn new(
        title: impl Into<String>,
        category: impl Into<String>,
        target_setting: impl Into<String>,
        items: Vec<SelectModalItem>,
        selected_id: Option<&str>,
    ) -> Self {
        let selected = if let Some(id) = selected_id {
            items.iter().position(|i| i.id == id).unwrap_or(0)
        } else {
            0
        };
        Self {
            title: title.into(),
            category: category.into(),
            items,
            selected,
            target_setting: target_setting.into(),
        }
    }

    pub fn next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.items.is_empty() {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_item(&self) -> Option<&SelectModalItem> {
        self.items.get(self.selected)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputModalState {
    pub title: String,
    pub prompt: String,
    pub value: String,
    pub target_setting: String,
}

impl InputModalState {
    pub fn new(
        title: impl Into<String>,
        prompt: impl Into<String>,
        default_val: impl Into<String>,
        target_setting: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            prompt: prompt.into(),
            value: default_val.into(),
            target_setting: target_setting.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    CommandPalette(CommandPaletteState),
    FilePicker(FilePickerState),
    ProfilePicker(ProfilePickerState),
    SelectModal(SelectModalState),
    InputModal(InputModalState),
    Help,
    Confirm { message: String, action: String },
}

impl Overlay {
    pub fn is_none(&self) -> bool {
        matches!(self, Overlay::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_palette_filtering() {
        let mut cp = CommandPaletteState::new();
        assert_eq!(cp.filtered_items().len(), 14);
        cp.query = "flash".to_string();
        let filtered = cp.filtered_items();
        assert!(filtered.len() >= 2);
        assert!(filtered.iter().any(|i| i.id.contains("flash")));
    }

    #[test]
    fn test_file_picker_navigation() {
        let mut fp = FilePickerState::new(".");
        if !fp.entries.is_empty() {
            fp.next();
            assert!(fp.selected < fp.entries.len());
            fp.prev();
            assert_eq!(fp.selected, 0);
        }
    }
}
