use std::marker::PhantomData;

use crate::{connection::{RawQuery, Queryable, Executable}, IntoSqlite};

use super::{Model, query::{ModelQueryFilter, ColumnQueryOrder}};

pub struct ModelDeleteQuery<M: Model> {
    query: RawQuery,
    model: PhantomData<M>,
}

impl<M: Model> ModelDeleteQuery<M> {
    pub fn new() -> Self {
        ModelDeleteQuery {
            query: RawQuery::new(format!("DELETE FROM {}", M::table_name()), Vec::new()),
            model: PhantomData,
        }
    }

    pub fn combine(self, other: RawQuery) -> Self {
        let mut params_old = self.query.params;
        params_old.extend(other.params);
        ModelDeleteQuery {
            query: RawQuery::new(format!("{} {}", self.query.sql, other.sql), params_old),
            model: PhantomData,
        }
    }

    // Filters
    pub fn filter(self, mut filter: impl ModelQueryFilter) -> Self {
        let mut filter_query = filter.get_query();
        filter_query.sql = format!("WHERE {}", filter_query.sql);
        ModelDeleteQuery::combine(self, filter_query)
    }

    // Limit and offset
    /// Limit the number of rows returned by the query.
    /// WARNING: This requires SQLITE_ENABLE_UPDATE_DELETE_LIMIT to be enabled in the sqlite3 library.
    pub fn limit(self, limit: u32) -> Self {
        self.combine(RawQuery::new("LIMIT ?".to_string(), vec![Box::new(limit)]))
    }

    /// Offset the number of rows returned by the query.
    pub fn offset(self, offset: u32) -> Self {
        self.combine(RawQuery::new("OFFSET ?".to_string(), vec![Box::new(offset)]))
    }

    // Order
    /// Order the rows returned by the query.
    /// WARNING: This requires SQLITE_ENABLE_UPDATE_DELETE_LIMIT to be enabled in the sqlite3 library.
    pub fn order_by(self, order: ColumnQueryOrder) -> Self {
        self.combine(RawQuery::new(format!("ORDER BY {}", order.into_sqlite()), Vec::new()))
    }
}

impl<M: Model> Queryable<()> for ModelDeleteQuery<M> {
    fn get_query(&mut self) -> RawQuery {
        self.query.move_clone()
    }

    fn parse_result(&mut self, _rows: rusqlite::Rows) -> () {
        // Nothing to parse
    }

    fn should_execute(&self) -> bool {
        true
    }
}

impl<M: Model> Executable<usize> for ModelDeleteQuery<M> {
    fn exec(self, conn: &crate::prelude::Connection) -> Result<usize, rusqlite::Error> {
        conn.exec(self)
    }
}