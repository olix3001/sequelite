pub mod model;
pub mod sql_types;
pub mod connection;

pub mod prelude {
    pub use crate::model::{Model, Column, ModelExt, SqliteRows, SqliteToSql,
        query::{ColumnQueryFilterImpl}
    };

    pub use crate::connection::Connection;
    pub use crate::connection::Executable;

    pub use sequelite_macro::Model;

    pub use rusqlite::Error as SqliteError;
}

extern crate rusqlite;

pub trait IntoSqlite {
    fn into_sqlite(&self) -> String;
}
pub trait IntoSqliteTy {
    fn into_sqlite() -> String;
}