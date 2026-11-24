//! Хранение записей в JSON-файле в каталоге данных пользователя
//! (например, ~/.local/share/supernote/notes.json на Linux).

use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Entry {
    pub title: String,
    pub value: String,
    #[serde(default)]
    pub info: String,
    /// Название группы; пустая строка — запись без группы.
    #[serde(default)]
    pub group: String,
}

/// Формат записи в файле. Поддерживает старый формат (одна строка).
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredEntry {
    Full(Entry),
    Legacy(String),
}

impl From<StoredEntry> for Entry {
    fn from(e: StoredEntry) -> Self {
        match e {
            StoredEntry::Full(e) => e,
            StoredEntry::Legacy(s) => Entry {
                title: s.clone(),
                value: s,
                info: String::new(),
                group: String::new(),
            },
        }
    }
}

#[derive(Deserialize)]
struct DataIn {
    entries: Vec<StoredEntry>,
}

#[derive(Serialize)]
struct DataOut<'a> {
    entries: &'a [Entry],
}

fn data_path() -> io::Result<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "не найден каталог данных"))?
        .join("supernote");
    Ok(dir.join("notes.json"))
}

pub fn load() -> io::Result<Vec<Entry>> {
    let path = data_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => {
            let data: DataIn = serde_json::from_str(&text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            Ok(data.entries.into_iter().map(Entry::from).collect())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

pub fn save(entries: &[Entry]) -> io::Result<()> {
    let path = data_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&DataOut { entries }).map_err(io::Error::other)?;
    // Пишем во временный файл и переименовываем, чтобы не потерять данные при сбое.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(tmp, path)
}
