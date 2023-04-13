use std::{marker::PhantomData, fmt::Debug, ops::{BitAnd, BitOr}};

use rusqlite::ToSql;

use crate::connection::{Queryable, RawQuery, IntoInsertable, Insertable, Executable};

use super::{Model, column::Column};

pub struct CountQuery;

pub struct ModelQuery<M> {
    model: PhantomData<M>,
    query: String,
    params: Vec<Box<dyn ToSql>>,
}

impl<M: Model> Debug for ModelQuery<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelQuery")
            .field("query", &self.query)
            .finish()
    }
}

// Only tables can be queried
impl<M: Model> ModelQuery<M> {
    // ====< Constructors >====
    pub fn select() -> Self {
        let query = format!("SELECT * FROM {}", M::table_name());
        ModelQuery {
            model: PhantomData,
            query,
            params: Vec::new(),
        }
    }

    pub fn count() -> ModelQuery<CountQuery> {
        let query = format!("SELECT COUNT(*) FROM {}", M::table_name());
        ModelQuery {
            model: PhantomData,
            query,
            params: Vec::new(),
        }
    }
}

// Every ModelQuery is a Queryable
impl<M> ModelQuery<M> {

    // ====< Utils >====
    pub fn combine(self, query: String, params: Vec<Box<dyn ToSql>>) -> Self {
        let mut params_old = self.params;
        params_old.extend(params);
        ModelQuery {
            model: PhantomData,
            query: format!("{} {}", self.query, query),
            params: params_old,
        }
    }

    // ====< Additional Methods >====
    pub fn filter(self, mut filter: impl ModelQueryFilter) -> Self {
        let filter_query = filter.get_query();
        ModelQuery::combine(self, format!("WHERE {}", filter_query.sql), filter_query.params)
    }

    pub fn limit(self, limit: u32) -> Self {
        ModelQuery::combine(self, "LIMIT ?".to_string(), vec![Box::new(limit)])
    }

    pub fn offset(self, offset: u32) -> Self {
        ModelQuery::combine(self, "OFFSET ?".to_string(), vec![Box::new(offset)])
    }
}

impl<M: Model> Queryable<Vec<M>> for ModelQuery<M> {
    fn get_query(&mut self) -> crate::connection::RawQuery {
        crate::connection::RawQuery::new(self.query.clone(), self.params.drain(..).collect())
    }

    fn parse_result(&mut self, rows: rusqlite::Rows) -> Vec<M> {
        M::parse_rows(rows)
    }
}

impl<M: Model> Executable<Vec<M>> for ModelQuery<M> {
    fn exec(self, conn: &crate::prelude::Connection) -> Result<Vec<M>, rusqlite::Error> {
        conn.query(self)
    }
}

impl Queryable<usize> for ModelQuery<CountQuery> {
    fn get_query(&mut self) -> crate::connection::RawQuery {
        crate::connection::RawQuery::new(self.query.clone(), self.params.drain(..).collect())
    }

    fn parse_result(&mut self, mut rows: rusqlite::Rows) -> usize {
        rows.next().unwrap().unwrap().get(0).unwrap()
    }
}

impl Executable<usize> for ModelQuery<CountQuery> {
    fn exec(self, conn: &crate::prelude::Connection) -> Result<usize, rusqlite::Error> {
        conn.query(self)
    }
}

pub trait ModelQueryFilter {
    fn get_query(&mut self) -> crate::connection::RawQuery;
}

pub struct ColumnQueryFilter {
    column: String,
    value: Option<Box<dyn ToSql>>,
    op: &'static str,
}

impl ModelQueryFilter for ColumnQueryFilter {
    fn get_query(&mut self) -> RawQuery {
        let sql = format!("{} {} ?", self.column, self.op);
        let params = vec![self.value.take().unwrap()];
        RawQuery::new(sql, params)
    }
}

pub struct ColumnQueryFilterUnary {
    column: String,
    op: &'static str,
}

impl ModelQueryFilter for ColumnQueryFilterUnary {
    fn get_query(&mut self) -> RawQuery {
        let sql = format!("{} {}", self.column, self.op);
        RawQuery::new(sql, Vec::new())
    }
}

macro_rules! trait_column_filter {
    ($fn:ident) => {
        fn $fn<V: ToSql + 'static>(self, value: V) -> ColumnQueryFilter;
    };
}

macro_rules! impl_column_filter {
    ($fn:ident, $op:literal) => {
        fn $fn<V: ToSql + 'static>(self, value: V) -> ColumnQueryFilter {
            ColumnQueryFilter {
                column: self.name(),
                op: $op,
                value: Some(Box::new(value)),
            }
        }
    };
}

pub trait ColumnQueryFilterImpl {
    trait_column_filter!(eq);
    trait_column_filter!(ne);
    trait_column_filter!(gt);
    trait_column_filter!(lt);
    trait_column_filter!(ge);
    trait_column_filter!(le);
    
    trait_column_filter!(like);
    trait_column_filter!(not_like);

    fn is_null(self) -> ColumnQueryFilterUnary;
    fn is_not_null(self) -> ColumnQueryFilterUnary;

    fn in_(self, values: Vec<impl ToSql>) -> ColumnQueryFilter;
    fn not_in(self, values: Vec<impl ToSql>) -> ColumnQueryFilter;
}

impl ColumnQueryFilterImpl for Column<'_> {
    impl_column_filter!(eq, "=");
    impl_column_filter!(ne, "!=");
    impl_column_filter!(gt, ">");
    impl_column_filter!(lt, "<");
    impl_column_filter!(ge, ">=");
    impl_column_filter!(le, "<=");

    impl_column_filter!(like, "LIKE");
    impl_column_filter!(not_like, "NOT LIKE");

    /// Check if the column is null (only for nullable columns)
    fn is_null(self) -> ColumnQueryFilterUnary {
        ColumnQueryFilterUnary {
            column: self.name(),
            op: "IS NULL",
        }
    }

    /// Check if the column is not null (only for nullable columns)
    fn is_not_null(self) -> ColumnQueryFilterUnary {
        ColumnQueryFilterUnary {
            column: self.name(),
            op: "IS NOT NULL",
        }
    }

    /// Check if the column is in the list of values
    fn in_(self, values: Vec<impl ToSql>) -> ColumnQueryFilter {
        let mut params = Vec::new();
        let mut sql = format!("{} IN (", self.name());
        for value in values {
            sql.push_str("?, ");
            params.push(Box::new(value));
        }
        sql.pop();
        sql.pop();
        sql.push(')');

        ColumnQueryFilter {
            column: self.name(),
            op: "",
            value: None,
        }
    }

    /// Check if the column is not in the list of values
    fn not_in(self, values: Vec<impl ToSql>) -> ColumnQueryFilter {
        let mut params = Vec::new();
        let mut sql = format!("{} NOT IN (", self.name());
        for value in values {
            sql.push_str("?, ");
            params.push(Box::new(value));
        }
        sql.pop();
        sql.pop();
        sql.push(')');

        ColumnQueryFilter {
            column: self.name(),
            op: "",
            value: None,
        }
    }
}


pub trait ModelQueryFilterExt: ModelQueryFilter {
    fn and<F: ModelQueryFilter>(self, filter: F) -> ModelQueryFilterAnd<Self, F>
    where
        Self: Sized;

    fn or<F: ModelQueryFilter>(self, filter: F) -> ModelQueryFilterOr<Self, F>
    where
        Self: Sized;
}

impl<F: ModelQueryFilter> ModelQueryFilterExt for F {
    fn and<F1: ModelQueryFilter>(self, filter: F1) -> ModelQueryFilterAnd<Self, F1>
    where
        Self: Sized,
    {
        ModelQueryFilterAnd {
            filter0: self,
            filter1: filter,
        }
    }

    fn or<F1: ModelQueryFilter>(self, filter: F1) -> ModelQueryFilterOr<Self, F1>
    where
        Self: Sized,
    {
        ModelQueryFilterOr {
            filter0: self,
            filter1: filter,
        }
    }
}

pub struct ModelQueryFilterAnd<F0: ModelQueryFilter, F1: ModelQueryFilter> {
    filter0: F0,
    filter1: F1,
}

pub struct ModelQueryFilterOr<F0: ModelQueryFilter, F1: ModelQueryFilter> {
    filter0: F0,
    filter1: F1,
}

impl<F0: ModelQueryFilter, F1: ModelQueryFilter> ModelQueryFilter for ModelQueryFilterAnd<F0, F1> {
    fn get_query(&mut self) -> crate::connection::RawQuery {
        let mut query = self.filter0.get_query();
        let mut query1 = self.filter1.get_query();
        query.sql = format!("{} AND {}", query.sql, query1.sql);
        query.params.append(&mut query1.params);
        query
    }
}

impl<F0: ModelQueryFilter, F1: ModelQueryFilter> ModelQueryFilter for ModelQueryFilterOr<F0, F1> {
    fn get_query(&mut self) -> crate::connection::RawQuery {
        let mut query = self.filter0.get_query();
        let mut query1 = self.filter1.get_query();
        query.sql = format!("{} OR {}", query.sql, query1.sql);
        query.params.append(&mut query1.params);
        query
    }
}

macro_rules! impl_op {
    ($op:ident ($fn:ident), $target:ident => $result:ident) => {
        impl<T: ModelQueryFilter> $op<T> for $target {
            type Output = $result<Self, T>;

            fn $fn(self, rhs: T) -> Self::Output {
                $result {
                    filter0: self,
                    filter1: rhs,
                }
            }
        }
    };
}

impl_op!(BitAnd (bitand), ColumnQueryFilter => ModelQueryFilterAnd);
impl_op!(BitOr (bitor), ColumnQueryFilter => ModelQueryFilterOr);

impl_op!(BitAnd (bitand), ColumnQueryFilterUnary => ModelQueryFilterAnd);
impl_op!(BitOr (bitor), ColumnQueryFilterUnary => ModelQueryFilterOr);


pub struct ModelInsertQuery<M: Model> {
    model: PhantomData<M>,
    columns: Vec<String>,
    values: Vec<Vec<Box<dyn ToSql>>>,
}

impl<M: Model> Insertable for ModelInsertQuery<M> {
    fn get_query(&mut self) -> RawQuery {
        let mut sql = format!("INSERT INTO {} (", M::table_name());
        for column in &self.columns {
            sql.push_str(column);
            sql.push_str(", ");
        }
        sql.pop();
        sql.pop();
        sql.push_str(") VALUES ");

        let mut params = Vec::new();
        for values in &mut self.values {
            sql.push('(');
            for value in values.drain(..) {
                sql.push_str("?, ");
                params.push(value);
            }
            sql.pop();
            sql.pop();
            sql.push(')');
            sql.push(',');
        }
        sql.pop();

        RawQuery {
            sql,
            params,
        }
    }
}

impl<M: Model> IntoInsertable for M {
    type Insertable = ModelInsertQuery<M>;

    fn into_insertable(&self) -> ModelInsertQuery<M> {
        let mut columns = Vec::new();
        let mut values = Vec::new();
        for column in M::columns() {
            let cv = self.column_value(column);

            if !column.can_insert_null() && cv.is_none() {
                panic!("Column '{}' is not nullable", column.name());
            }

            if let Some(value) = cv {
                columns.push(column.name().to_string());
                values.push(value);
            }
        }

        ModelInsertQuery {
            model: PhantomData,
            columns,
            values: vec![values], // Only one row
        }
    }
}

impl<M: Model, const N: usize, I: IntoInsertable<Insertable = ModelInsertQuery<M>>> IntoInsertable for &[I; N] {
    type Insertable = ModelInsertQuery<M>;

    fn into_insertable(&self) -> Self::Insertable {
        let mut columns = Vec::new();
        let mut values = Vec::new();
        
        for v in self.iter() {
            let mut insertable = v.into_insertable();
           
            if columns.is_empty() {
                columns.append(&mut insertable.columns);
            } else {
                assert_eq!(columns, insertable.columns);
            }

            let v = insertable.values.pop().unwrap();
            values.push(v);
        }

        ModelInsertQuery {
            model: PhantomData,
            columns,
            values,
        }
    }
}

impl<M: Model, I: IntoInsertable<Insertable = ModelInsertQuery<M>>> IntoInsertable for &[I] {
    type Insertable = ModelInsertQuery<M>;

    fn into_insertable(&self) -> Self::Insertable {
        let mut columns = Vec::new();
        let mut values = Vec::new();
        
        for v in self.iter() {
            let mut insertable = v.into_insertable();
            
            if columns.is_empty() {
                columns.append(&mut insertable.columns);
            } else {
                assert_eq!(columns, insertable.columns);
            }

            let v = insertable.values.pop().unwrap();
            values.push(v);
        }

        ModelInsertQuery {
            model: PhantomData,
            columns,
            values,
        }
    }
}