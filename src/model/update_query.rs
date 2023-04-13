use rusqlite::ToSql;

use crate::connection::{RawQuery, Queryable, Executable};

use super::{Model, Column, query::ModelQueryFilter};

pub struct ModelUpdateQuery<T: Model> {
    pub query: RawQuery,
    pub columns: Vec<Column<'static>>,
    pub values: Vec<Box<dyn rusqlite::types::ToSql>>,
    marker: std::marker::PhantomData<T>
}

impl<T: Model> ModelUpdateQuery<T> {
    pub fn new() -> Self {
        ModelUpdateQuery {
            query: RawQuery::new("".to_string(), Vec::new()),
            columns: Vec::new(),
            values: Vec::new(),
            marker: Default::default()
        }
    }

    pub fn combine(self, other: RawQuery) -> Self {
        let mut params_old = self.query.params;
        params_old.extend(other.params);
        ModelUpdateQuery { 
            query: RawQuery::new(format!("{} {}", self.query.sql, other.sql), params_old),
            columns: self.columns,
            values: self.values,
            marker: Default::default()
        }
    }

    // Filters
    pub fn filter(self, mut filter: impl ModelQueryFilter) -> Self {
        let mut filter_query = filter.get_query();
        filter_query.sql = format!("WHERE {}", filter_query.sql);
        ModelUpdateQuery::combine(self, filter_query)
    }

    // Limit and offset
    pub fn limit(self, limit: u32) -> Self {
        self.combine(RawQuery::new("LIMIT ?".to_string(), vec![Box::new(limit)]))
    }
    pub fn offset(self, offset: u32) -> Self {
        self.combine(RawQuery::new("OFFSET ?".to_string(), vec![Box::new(offset)]))
    }

    // Update value for a column
    pub fn set<V: ToSql + 'static>(self, column: Column<'static>, value: V) -> Self {
        let mut columns = self.columns;
        let mut values = self.values;

        columns.push(column);
        values.push(Box::new(value));
        ModelUpdateQuery {
            query: self.query,
            columns,
            values,
            marker: Default::default()
        }
    }

    // TODO: Add support for multiple values in one function
}

impl<M: Model> Queryable<()> for ModelUpdateQuery<M> {
    fn get_query(&mut self) -> RawQuery {
        let mut sql = format!("UPDATE {} SET ", M::table_name());

        // Set columns
        for (i, column) in self.columns.iter().enumerate() {
            sql = format!("{}{}=?", sql, column.name());
            if i != self.columns.len() - 1 {
                sql = format!("{}, ", sql);
            }
        }

        // Combine params
        let mut params = Vec::new();
        for value in self.values.drain(..) {
            params.push(value);
        }

        params.extend(self.query.params.drain(..));

        RawQuery::new(format!("{}{}", sql, self.query.sql), params)
    }

    fn parse_result(&mut self, _rows: rusqlite::Rows) -> () {
        ()
    }

    fn should_execute(&self) -> bool {
        true
    }
}

impl<M: Model> Executable<usize> for ModelUpdateQuery<M> {
    fn exec(self, conn: &crate::connection::Connection) -> Result<usize, rusqlite::Error> {
        conn.exec(self)
    }
}


