use crate::IntoSqlite;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteType {
    Integer,
    Text
}

impl IntoSqlite for i32 {
    fn into_sqlite(&self) -> String {
        self.to_string()
    }
}

impl IntoSqlite for &str {
    fn into_sqlite(&self) -> String {
        format!("'{}'", self)
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
            SqliteType::Text => "TEXT".to_string()
        }
    }
}

impl SqliteType {
    pub fn from_str(s: &str) -> Option<SqliteType> {
        match s.to_uppercase().as_str() {
            "INTEGER" => Some(SqliteType::Integer),
            "TEXT" => Some(SqliteType::Text),
            _ => None
        }
    }
}

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