use crate::connection::Connection;

use self::delete_query::ModelDeleteQuery;
use self::query::CountQuery;
use self::update_query::ModelUpdateQuery;

mod column;
pub mod migrator;
pub mod query;
pub mod update_query;
pub mod delete_query;
pub mod relation;

pub use rusqlite::Row as SqliteRow;
pub use rusqlite::Rows as SqliteRows;
pub use rusqlite::ToSql as SqliteToSql;
pub use column::Column;

/// A trait that needs to be implemented for all models that are used with sequelite.
/// 
/// This trait is hard to implement manually, so it is automatically implemented for every struct that derives [Model](sequelite_macro::Model).
/// 
/// ## Note
/// This trait is not meant to be implemented manually.
pub trait Model {
    fn table_name() -> &'static str;
    fn columns() -> &'static [Column<'static>];
    fn count_columns() -> usize;
    fn get_id(&self) -> i64;
    fn id_column() -> Column<'static>;
    fn column_value(&self, column: &'static Column<'static>) -> Option<Box<dyn rusqlite::types::ToSql>>;
    fn parse_rows(rows: rusqlite::Rows, offset: usize, joins: &Vec<String>) -> Vec<Self>
    where
        Self: Sized;
    fn parse_row(row: &rusqlite::Row, offset: usize, joins: &Vec<String>) -> Self
    where
        Self: Sized;
}

/// A trait that extends the [Model](Model) trait with some useful methods.
pub trait ModelExt<M: Model> {
    fn select() -> query::ModelQuery<M>
    where
        Self: Sized;

    fn insert(self, conn: &Connection) -> Result<i64, rusqlite::Error> 
    where
        Self: Sized;

    fn count() -> query::ModelQuery<CountQuery>
    where
        Self: Sized;

    fn update() -> ModelUpdateQuery<M>
    where
        Self: Sized;

    fn delete() -> ModelDeleteQuery<M>
    where
        Self: Sized;

}

impl<M: Model> ModelExt<M> for M {
    /// Creates a new [ModelQuery](query::ModelQuery) that can be used to select rows from the database.
    fn select() -> query::ModelQuery<M>
    where
        Self: Sized,
    {
        query::ModelQuery::select()
    }

    /// Inserts the model into the database.
    fn insert(self, conn: &Connection) -> Result<i64, rusqlite::Error> 
        where
            Self: Sized {
        conn.insert(self)
    }

    /// Creates a new [ModelQuery](query::ModelQuery) that can be used to count rows from the database.
    fn count() -> query::ModelQuery<CountQuery>
    where
        Self: Sized,
    {
        query::ModelQuery::<M>::count()
    }

    /// Creates a new [ModelUpdateQuery](update_query::ModelUpdateQuery) that can be used to update rows in the database.
    fn update() -> ModelUpdateQuery<M>
    where
        Self: Sized,
    {
        ModelUpdateQuery::new()
    }

    /// Creates a new [ModelDeleteQuery](delete_query::ModelDeleteQuery) that can be used to delete rows from the database.
    fn delete() -> ModelDeleteQuery<M>
    where
        Self: Sized,
    {
        ModelDeleteQuery::new()
    }
}