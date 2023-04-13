use crate::connection::Connection;

use self::query::CountQuery;
use self::update_query::ModelUpdateQuery;

mod column;
pub mod migrator;
pub mod query;
pub mod update_query;

pub use rusqlite::Rows as SqliteRows;
pub use rusqlite::ToSql as SqliteToSql;
pub use column::Column;

pub trait Model {
    fn table_name() -> &'static str;
    fn columns() -> &'static [Column<'static>];
    fn column_value(&self, column: &'static Column<'static>) -> Option<Box<dyn rusqlite::types::ToSql>>;
    fn parse_rows(rows: rusqlite::Rows) -> Vec<Self>
    where
        Self: Sized;
}

// impl<M: Model> IntoSqliteTy for M {
//     fn into_sqlite() -> String {
//         let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (", M::table_name());
//         for (i, column) in M::columns().iter().enumerate() {
//             sql = format!("{}{}", sql, column.into_sqlite().0);
//             if i != M::columns().len() - 1 {
//                 sql = format!("{}, ", sql);
//             }
//         }
//         format!("{})", sql)
//     }
// }

pub trait ModelExt<M: Model> {
    fn select() -> query::ModelQuery<M>
    where
        Self: Sized;

    fn insert(self, conn: &Connection) -> Result<(), rusqlite::Error> 
    where
        Self: Sized;

    fn count() -> query::ModelQuery<CountQuery>
    where
        Self: Sized;

    fn update() -> ModelUpdateQuery<M>
    where
        Self: Sized;

}

impl<M: Model> ModelExt<M> for M {
    fn select() -> query::ModelQuery<M>
    where
        Self: Sized,
    {
        query::ModelQuery::select()
    }

    fn insert(self, conn: &Connection) -> Result<(), rusqlite::Error> 
        where
            Self: Sized {
        conn.insert(self)
    }

    fn count() -> query::ModelQuery<CountQuery>
    where
        Self: Sized,
    {
        query::ModelQuery::<M>::count()
    }

    fn update() -> ModelUpdateQuery<M>
    where
        Self: Sized,
    {
        ModelUpdateQuery::new()
    }
}