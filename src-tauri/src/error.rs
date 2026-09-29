use serde::Serialize;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::new("DB", format!("Sorter не смог прочитать или записать свою базу ({e}). Перезапусти Sorter и попробуй снова."))
    }
}

pub fn file_label(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

pub fn io_error(e: &io::Error, path: &Path) -> AppError {
    let name = file_label(path);
    match e.raw_os_error() {
        Some(32) | Some(33) => AppError::new(
            "FILE_BUSY",
            format!("Файл «{name}» открыт в другой программе. Закрой её и попробуй снова."),
        ),
        Some(2) | Some(3) => AppError::new(
            "FILE_NOT_THERE",
            format!("Файла «{name}» нет на месте — возможно, его переместили или удалили."),
        ),
        Some(112) | Some(39) => AppError::new("DISK_FULL", "На диске стопки не хватает места. Освободи место и попробуй снова."),
        Some(5) => AppError::new("ACCESS_DENIED", format!("Нет доступа к «{name}». Проверь права на папку.")),
        Some(80) | Some(183) => AppError::new(
            "FILE_IN_THE_WAY",
            format!("На месте «{name}» уже лежит другой файл — убери его и попробуй снова."),
        ),
        _ => AppError::new("IO", format!("Не получилось переместить «{name}» ({e}). Попробуй ещё раз; если повторится — проверь диск и права на папку.")),
    }
}
