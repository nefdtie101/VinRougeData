use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileBrowser {
    pub current_dir: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected_index: usize,
    pub filter: FileFilter,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileFilter {
    All,
    Csv,
    Excel,
}

impl FileBrowser {
    pub fn new(filter: FileFilter) -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut browser = Self {
            current_dir: current_dir.clone(),
            entries: Vec::new(),
            selected_index: 0,
            filter,
        };
        browser.refresh();
        browser
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        self.selected_index = 0;

        // Add parent directory entry if not at root
        if self.current_dir.parent().is_some() {
            self.entries.push(FileEntry {
                path: self.current_dir.parent().unwrap().to_path_buf(),
                name: "..".to_string(),
                is_dir: true,
                size: None,
            });
        }

        // Read directory entries
        if let Ok(read_dir) = fs::read_dir(&self.current_dir) {
            let mut entries: Vec<FileEntry> = read_dir
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    let metadata = entry.metadata().ok()?;
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Skip hidden files
                    if name.starts_with('.') && name != ".." {
                        return None;
                    }

                    let is_dir = metadata.is_dir();

                    // Apply filter for files
                    if !is_dir {
                        match self.filter {
                            FileFilter::Csv => {
                                if !name.to_lowercase().ends_with(".csv") {
                                    return None;
                                }
                            }
                            FileFilter::Excel => {
                                let lower = name.to_lowercase();
                                if !lower.ends_with(".xlsx")
                                    && !lower.ends_with(".xls")
                                    && !lower.ends_with(".xlsm")
                                {
                                    return None;
                                }
                            }
                            FileFilter::All => {}
                        }
                    }

                    Some(FileEntry {
                        path: path.clone(),
                        name,
                        is_dir,
                        size: if is_dir { None } else { Some(metadata.len()) },
                    })
                })
                .collect();

            // Sort: directories first, then files, alphabetically
            entries.sort_by(|a, b| {
                if a.is_dir && !b.is_dir {
                    std::cmp::Ordering::Less
                } else if !a.is_dir && b.is_dir {
                    std::cmp::Ordering::Greater
                } else {
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                }
            });

            self.entries.extend(entries);
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index < self.entries.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    pub fn enter_selected(&mut self) -> Option<PathBuf> {
        if let Some(entry) = self.entries.get(self.selected_index) {
            if entry.is_dir {
                self.current_dir = entry.path.clone();
                self.refresh();
                None
            } else {
                Some(entry.path.clone())
            }
        } else {
            None
        }
    }

    pub fn go_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.refresh();
        }
    }

    pub fn get_selected_path(&self) -> Option<&Path> {
        self.entries.get(self.selected_index).map(|e| e.path.as_path())
    }

    pub fn format_size(size: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if size >= GB {
            format!("{:.2} GB", size as f64 / GB as f64)
        } else if size >= MB {
            format!("{:.2} MB", size as f64 / MB as f64)
        } else if size >= KB {
            format!("{:.2} KB", size as f64 / KB as f64)
        } else {
            format!("{} B", size)
        }
    }
}
