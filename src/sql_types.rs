use crate::IntoSqlite;

/// The type of a column in a SQLite database.
/// 
/// This is used to determine the type of a column when creating a table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SqliteType {
    Integer,
    Real,
    Text,
    Blob,
    DateTime,
}

impl IntoSqlite for i32 {
    fn into_sqlite(&self) -> String {
        self.to_string()
    }
}

impl IntoSqlite for i64 {
    fn into_sqlite(&self) -> String {
        self.to_string()
    }
}

impl IntoSqlite for f32 {
    fn into_sqlite(&self) -> String {
        self.to_string()
    }
}

impl IntoSqlite for f64 {
    fn into_sqlite(&self) -> String {
        self.to_string()
    }
}

impl IntoSqlite for bool {
    fn into_sqlite(&self) -> String {
        match self {
            true => "1".to_string(),
            false => "0".to_string()
        }
    }
}

impl IntoSqlite for &str {
    fn into_sqlite(&self) -> String {
        format!("'{}'", self)
    }
}

impl IntoSqlite for chrono::NaiveDateTime {
    fn into_sqlite(&self) -> String {
        let date_str = self.format("%F %T").to_string();
        format!("'{}'", date_str)
    }
}

pub struct NowTime;
impl IntoSqlite for NowTime {
    fn into_sqlite(&self) -> String {
        "CURRENT_TIMESTAMP".to_string()
    }
}

impl IntoSqlite for String {
    fn into_sqlite(&self) -> String {
        format!("'{}'", self)
    }
}

impl IntoSqlite for SqliteType {
    fn into_sqlite(&self) -> String {
        match self {
            SqliteType::Integer => "INTEGER".to_string(),
            SqliteType::Text => "TEXT".to_string(),
            SqliteType::Real => "REAL".to_string(),
            SqliteType::Blob => "BLOB".to_string(),
            SqliteType::DateTime => "DATETIME".to_string()
        }
    }
}

impl SqliteType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<SqliteType> {
        match s.to_uppercase().as_str() {
            "INTEGER" => Some(SqliteType::Integer),
            "TEXT" => Some(SqliteType::Text),
            "REAL" => Some(SqliteType::Real),
            "BLOB" => Some(SqliteType::Blob),
            "DATETIME" => Some(SqliteType::DateTime),
            _ => None
        }
    }
}

/// A flag for a column in a SQLite database.
/// 
/// This is used to determine the flags of a column when creating a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqliteFlag {
    PrimaryKey,
    NotNull,
    Unique,
    AutoIncrement,
}

impl IntoSqlite for SqliteFlag {
    fn into_sqlite(&self) -> String {
        match self {
            SqliteFlag::PrimaryKey => "PRIMARY KEY".to_string(),
            SqliteFlag::NotNull => "NOT NULL".to_string(),
            SqliteFlag::Unique => "UNIQUE".to_string(),
            SqliteFlag::AutoIncrement => "AUTOINCREMENT".to_string()
        }
    }
}