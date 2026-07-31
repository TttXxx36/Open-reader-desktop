use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database lock is unavailable")]
    Lock,
}

pub struct Database {
    connection: Mutex<Connection>,
}

#[derive(Debug, Serialize)]
pub struct LibrarySummary {
    pub book_count: i64,
    pub last_opened: Option<String>,
}

impl Database {
    pub fn open(app_data_dir: &Path) -> Result<Self, DbError> {
        fs::create_dir_all(app_data_dir)?;
        let database_path: PathBuf = app_data_dir.join("open-reader.db");
        let connection = Connection::open(database_path)?;
        connection.execute_batch(include_str!("../migrations/0001_init.sql"))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn summary(&self) -> Result<LibrarySummary, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let book_count = connection.query_row(
            "SELECT COUNT(*) FROM books",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let last_opened = connection
            .query_row("SELECT MAX(updated_at) FROM books", [], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()?
            .flatten();

        Ok(LibrarySummary {
            book_count,
            last_opened,
        })
    }
}
