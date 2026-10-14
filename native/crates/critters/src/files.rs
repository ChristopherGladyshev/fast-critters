//! Содержимое стилей, читается один раз на процесс: результат сборки неизменен для BUILD_ID.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const MAX_FILES: usize = 4096;

#[derive(Default)]
pub struct FileCache {
    files: Mutex<HashMap<String, Arc<str>>>,
    enabled: bool,
}

impl FileCache {
    pub fn new(enabled: bool) -> Self {
        FileCache {
            files: Mutex::default(),
            enabled,
        }
    }

    /// `fs.readFile(filename, 'utf8')`; ошибки не кэшируются.
    pub fn read(&self, filename: &str) -> std::io::Result<Arc<str>> {
        if self.enabled
            && let Some(content) = self.lock().get(filename)
        {
            return Ok(content.clone());
        }
        let bytes = std::fs::read(filename)?;
        let content: Arc<str> = String::from_utf8_lossy(&bytes).into();
        if self.enabled {
            let mut files = self.lock();
            if files.len() >= MAX_FILES {
                files.clear();
            }
            files.insert(filename.to_owned(), content.clone());
        }
        Ok(content)
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<str>>> {
        // Паника под локом оставляет map консистентной; продолжаем её использовать.
        self.files
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
