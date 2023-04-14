use std::{marker::PhantomData, fmt::Debug, ops::{BitAnd, BitOr}};

use rusqlite::ToSql;

use crate::{connection::{Queryable, RawQuery, IntoInsertable, Insertable, Executable}, IntoSqlite};

use super::{Model, column::Column};

/// Just a marker type for count queries
pub struct CountQuery;

/// A trait for filtering queries
/// 
/// This allows you to filter, limit, offset, and order elements that you are querying.
/// 
/// # Example
/// ```rs
/// User::select()
///     .filter(User::id.eq(1) & User::name.like("%test%"))
///     .limit(1)
///     .order(User::id.desc())
///     .exec(&conn).unwrap();
/// ```
pub struct ModelQuery<M> {
    model: PhantomData<M>,
    table_name: String,
    query: String,
    joins: Vec<String>,
    params: Vec<Box<dyn ToSql>>,
}

impl<M: Model> Debug for ModelQuery<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelQuery")
            .field("query", &self.query)
            .finish()
    }
}

impl<M> Default for ModelQuery<M> {
    fn default() -> Self {
        Self {
            model: Default::default(),
            table_name: "unknown".to_string(),
            query: String::new(),
            joins: Vec::new(),
            params: Vec::new(),
        }
    }
}

// Only tables can be queried
impl<M: Model> ModelQuery<M> {
    // ====< Constructors >====
    pub fn select() -> Self {
        let query = format!("SELECT * FROM {}", M::table_name());
        ModelQuery {
            model: PhantomData,
            table_name: M::table_name().to_string(),
            query,
            ..Default::default()
        }
    }

    pub fn count() -> ModelQuery<CountQuery> {
        let query = format!("SELECT COUNT(*) FROM {}", M::table_name());
        ModelQuery {
            model: PhantomData,
            table_name: M::table_name().to_string(),
            query,
            ..Default::default()
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
            table_name: self.table_name,
            query: format!("{} {}", self.query, query),
            joins: self.joins,
            params: params_old,
        }
    }

    // ====< Additional Methods >====
    /// Filter the query with the given filter
    /// 
    /// ## Arguments
    /// * `filter` - The filter to apply to the query
    /// 
    /// ## Example
    /// ```rs
    /// let user = User::select()
    ///     .filter(User::id.eq(1) & User::name.like("%test%"))
    ///     .exec(&conn).unwrap();
    /// ```
    pub fn filter(self, mut filter: impl ModelQueryFilter) -> Self {
        let filter_query = filter.get_query();
        ModelQuery::combine(self, format!("WHERE {}", filter_query.sql), filter_query.params)
    }

    /// Select element by id
    /// 
    /// ## Arguments
    /// * `id` - The id of the element to select
    /// 
    /// ## Example
    /// ```rs
    /// let user = User::select()
    ///     .with_id(1)
    ///     .exec(&conn).unwrap();
    /// ```
    /// 
    /// ## Note
    /// This is equivalent to `.filter(User::id.eq(id)).limit(1)` and should not be combined with other filters or limits.
    pub fn with_id(self, id: i64) -> Self {
        let table_name = self.table_name.clone();
        ModelQuery::combine(self, format!("WHERE {}.id = ? LIMIT 1", table_name), vec![Box::new(id)])
    }

    /// Limit the number of elements returned
    /// 
    /// ## Arguments
    /// * `limit` - The maximum number of elements to return
    /// 
    /// ## Example
    /// ```rs
    /// let users = User::select()
    ///     .limit(10)
    ///     .exec(&conn).unwrap();
    /// ```
    pub fn limit(self, limit: u32) -> Self {
        ModelQuery::combine(self, "LIMIT ?".to_string(), vec![Box::new(limit)])
    }

    /// Offset selection by the given number of elements
    /// 
    /// ## Arguments
    /// * `offset` - The number of elements to skip
    /// 
    /// ## Example
    /// ```rs
    /// let users = User::select()
    ///     .offset(10)
    ///     .exec(&conn).unwrap();
    /// ```
    pub fn offset(self, offset: u32) -> Self {
        ModelQuery::combine(self, "OFFSET ?".to_string(), vec![Box::new(offset)])
    }

    /// Order the elements by the given order
    /// 
    /// ## Arguments
    /// * `order` - The order to apply to the elements
    /// 
    /// ## Example
    /// ```rs
    /// let users = User::select()
    ///     .order(User::id.desc())
    ///     .exec(&conn).unwrap();
    /// ```
    pub fn order_by(self, order: ColumnQueryOrder) -> Self {
        ModelQuery::combine(self, format!("ORDER BY {}", order.into_sqlite()), Vec::new())
    }

    /// **WARNING:** This is highly experimental and may not work as expected
    /// Use Relation::get() or Relation::take() instead
    /// 
    /// ## Arguments
    /// * `relation` - The relation to join
    /// 
    /// ## Example
    /// ```rs
    /// let users = Post::select()
    ///     .join_relation(Post::author)
    ///     .exec(&conn).unwrap();
    /// ```
    pub fn join_relation(mut self, relation: Column<'static>) -> Self {
        // Ensure that the relation is a relation
        match relation.get_relation() {
            Some(relation) => {
                // Left join the relation table
                let query = format!("{} LEFT JOIN {} ON {}.{} = {}.{}", self.query, relation.table, relation.table, relation.foreign_key_column.name_const(), relation.local_table, relation.local_key_column_name );

                self.joins.push(relation.local_key_column_name.to_string());
                // Add the relation to the joins
                ModelQuery {
                    model: PhantomData,
                    table_name: self.table_name,
                    query,
                    joins: self.joins,
                    params: self.params,
                }
            },
            None => panic!("Cannot join a non-relation column"),
        }
    }

    /// Select only the given columns (do not use this if you want to map to a model column which is not an `Option<T>`)
    /// 
    /// ## Arguments
    /// * `columns` - The columns to select
    /// 
    /// ## Example
    /// ```rs
    /// let users = User::select()
    ///     .columns(&[User::id, User::name])
    ///     .exec(&conn).unwrap();
    /// ```
    pub fn columns(self, columns: &[Column<'static>]) -> Self {
        let columns = columns.iter().map(|c| c.name()).collect::<Vec<_>>().join(", ");
        // Replace first SELECT * with the given columns
        let query = self.query.replacen('*', &columns, 1);
        ModelQuery {
            model: PhantomData,
            table_name: self.table_name,
            query,
            joins: self.joins,
            params: self.params,
        }
    }
}

impl<M: Model> Queryable<Vec<M>> for ModelQuery<M> {
    fn get_query(&mut self) -> crate::connection::RawQuery {
        crate::connection::RawQuery::new(self.query.clone(), self.params.drain(..).collect())
    }

    fn parse_result(&mut self, rows: rusqlite::Rows) -> Vec<M> {
        M::parse_rows(rows, 0, &self.joins)
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

pub struct InQueryFilter {
    sql: RawQuery,
}

impl ModelQueryFilter for InQueryFilter {
    fn get_query(&mut self) -> RawQuery {
        self.sql.move_clone()
    }
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
    ($fn:ident, $op:literal, $doc:expr) => {
        #[doc = $doc]
        fn $fn<V: ToSql + 'static>(self, value: V) -> ColumnQueryFilter {
            ColumnQueryFilter {
                column: format!("{}.{}", self.table_name, self.name()),
                op: $op,
                value: Some(Box::new(value)),
            }
        }
    };

    ($fn:ident, $op:literal) => {
        impl_column_filter!($fn, $op, "There is no documentation for this filter");
    };
}

pub struct ColumnQueryOrder {
    column: String,
    order: ColumnQueryOrdering,
}

impl IntoSqlite for ColumnQueryOrder {
    fn into_sqlite(&self) -> String {
        format!("{} {}", self.column, self.order.into_sqlite())
    }
}

pub enum ColumnQueryOrdering {
    Ascending,
    Descending,
}

impl IntoSqlite for ColumnQueryOrdering {
    fn into_sqlite(&self) -> String {
        match self {
            ColumnQueryOrdering::Ascending => "ASC".to_string(),
            ColumnQueryOrdering::Descending => "DESC".to_string(),
        }
    }
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

    fn in_(self, values: impl ColumnInQuery) -> InQueryFilter;
    fn not_in(self, values: impl ColumnInQuery) -> InQueryFilter;

    fn asc(self) -> ColumnQueryOrder;
    fn desc(self) -> ColumnQueryOrder;
}

impl ColumnQueryFilterImpl for Column<'_> {
    impl_column_filter!(eq, "=", "
        Checks if the column is equal to the given value.
        ## Example
        ```rust
        User::select().filter(User::name.eq(\"John\")).exec(conn);
        ```
        This will generate the following SQL query:
        ```sql
        -- ? is a parameter
        SELECT * FROM users WHERE users.name = ?;
        ```
    ");
    impl_column_filter!(ne, "!=", "
        Checks if the column is not equal to the given value.
        ## Example
        ```rust
        User::select().filter(User::name.ne(\"John\")).exec(conn);
        ```
        This will generate the following SQL query:
        ```sql
        -- ? is a parameter
        SELECT * FROM users WHERE users.name != ?;
        ```
    ");
    impl_column_filter!(gt, ">", "
        Checks if the column is greater than the given value.
        ## Example
        ```rust
        User::select().filter(User::age.gt(18)).exec(conn);
        ```
        This will generate the following SQL query:
        ```sql
        -- ? is a parameter
        SELECT * FROM users WHERE users.age > ?;
        ```
    ");
    impl_column_filter!(lt, "<", "
        Checks if the column is less than the given value.
        ## Example
        ```rust
        User::select().filter(User::age.lt(18)).exec(conn);
        ```
        This will generate the following SQL query:
        ```sql
        -- ? is a parameter
        SELECT * FROM users WHERE users.age < ?;
        ```
    ");
    impl_column_filter!(ge, ">=", "
        Checks if the column is greater than or equal to the given value.
        ## Example
        ```rust
        User::select().filter(User::age.ge(18)).exec(conn);
        ```
        This will generate the following SQL query:
        ```sql
        -- ? is a parameter
        SELECT * FROM users WHERE users.age >= ?;
        ```
    ");
    impl_column_filter!(le, "<=", "
        Checks if the column is less than or equal to the given value.
        ## Example
        ```rust
        User::select().filter(User::age.le(18)).exec(conn);
        ```
        This will generate the following SQL query:
        ```sql
        -- ? is a parameter
        SELECT * FROM users WHERE users.age <= ?;
        ```
    ");

    impl_column_filter!(like, "LIKE", "
        Checks if the column is like the given value.
        ## Example
        ```rust
        User::select().filter(User::name.like(\"%John%\")).exec(conn);
        ```
        This will generate the following SQL query:
        ```sql
        -- ? is a parameter
        SELECT * FROM users WHERE users.name LIKE ?;
        ```
    ");
    impl_column_filter!(not_like, "NOT LIKE", "
        Checks if the column is not like the given value.
        ## Example
        ```rust
        User::select().filter(User::name.not_like(\"%John%\")).exec(conn);
        ```
        This will generate the following SQL query:
        ```sql
        SELECT * FROM users WHERE users.name NOT LIKE ?;
        ```
    ");

    /// Check if the column is null (only for nullable columns)
    /// ## Example
    /// ```rust
    /// User::select().filter(User::name.is_null()).exec(conn);
    /// ```
    /// This will generate the following SQL query:
    /// ```sql
    /// SELECT * FROM users WHERE users.name IS NULL;
    /// ```
    fn is_null(self) -> ColumnQueryFilterUnary {
        ColumnQueryFilterUnary {
            column: format!("{}.{}", self.table_name, self.name()),
            op: "IS NULL",
        }
    }

    /// Check if the column is not null (only for nullable columns)
    /// ## Example
    /// ```rust
    /// User::select().filter(User::name.is_not_null()).exec(conn);
    /// ```
    /// This will generate the following SQL query:
    /// ```sql
    /// SELECT * FROM users WHERE users.name IS NOT NULL;
    /// ```
    fn is_not_null(self) -> ColumnQueryFilterUnary {
        ColumnQueryFilterUnary {
            column: format!("{}.{}", self.table_name, self.name()),
            op: "IS NOT NULL",
        }
    }

    /// Check if the column is in the list of values
    /// ## Example
    /// ```rust
    /// User::select().filter(User::name.in_(vec!["John", "Jane"])).exec(conn);
    /// ```
    /// This will generate the following SQL query:
    /// ```sql
    /// -- ? is a parameter
    /// SELECT * FROM users WHERE users.name IN (?, ?);
    /// ```
    fn in_(self, values: impl ColumnInQuery) -> InQueryFilter {
        let q = values.to_query();
        let sql = format!("{}.{} IN {}", self.table_name, self.name(), q.sql);

        InQueryFilter { sql: RawQuery::new(sql, q.params) }
    }

    /// Check if the column is not in the list of values
    /// ## Example
    /// ```rust
    /// User::select().filter(User::name.not_in(vec!["John", "Jane"])).exec(conn);
    /// ```
    /// This will generate the following SQL query:
    /// ```sql
    /// -- ? is a parameter
    /// SELECT * FROM users WHERE users.name NOT IN (?, ?);
    /// ```
    fn not_in(self, values: impl ColumnInQuery) -> InQueryFilter {
        let q = values.to_query();
        let sql = format!("{}.{} NOT IN {}", self.table_name, self.name(), q.sql);

        InQueryFilter { sql: RawQuery::new(sql, q.params) }
    }

    /// Order the query by the column in ascending order
    /// ## Example
    /// ```rust
    /// User::select().order_by(User::name.asc()).exec(conn);
    /// ```
    /// This will generate the following SQL query:
    /// ```sql
    /// SELECT * FROM users ORDER BY users.name ASC;
    /// ```
    fn asc(self) -> ColumnQueryOrder {
        ColumnQueryOrder {
            column: self.name(),
            order: ColumnQueryOrdering::Ascending,
        }
    }

    /// Order the query by the column in descending order
    /// ## Example
    /// ```rust
    /// User::select().order_by(User::name.desc()).exec(conn);
    /// ```
    /// This will generate the following SQL query:
    /// ```sql
    /// SELECT * FROM users ORDER BY users.name DESC;
    /// ```
    fn desc(self) -> ColumnQueryOrder {
        ColumnQueryOrder {
            column: self.name(),
            order: ColumnQueryOrdering::Descending,
        }
    }
}

pub trait ColumnInQuery {
    fn to_query(self) -> RawQuery;
}

impl<M: Model> ColumnInQuery for ModelQuery<M> {
    fn to_query(mut self) -> RawQuery {
        let mut query = self.get_query();
        let sql = format!("({})", query.sql);
        query.sql = sql;
        query
    }
}

impl<T: ToSql + 'static> ColumnInQuery for Vec<T> {
    fn to_query(self) -> RawQuery {
        let mut params = Vec::new();
        let mut sql = String::from("(");

        for (i, v) in self.into_iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }

            sql.push('?');
            params.push(Box::new(v) as Box<dyn ToSql + 'static>);
        }

        sql.push(')');

        RawQuery::new(sql, params)
    }
}

impl<T: ToSql + 'static> ColumnInQuery for &'static [T] {
    fn to_query(self) -> RawQuery {
        let mut params = Vec::new();
        let mut sql = String::from("(");

        for (i, v) in self.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }

            sql.push('?');
            params.push(Box::new(v) as Box<dyn ToSql + 'static>);
        }

        sql.push(')');

        RawQuery::new(sql, params)
    }
}
impl<T: ToSql + 'static, const N: usize> ColumnInQuery for &'static [T; N] {
    fn to_query(self) -> RawQuery {
        let mut params = Vec::new();
        let mut sql = String::from("(");

        for (i, v) in self.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }

            sql.push('?');
            params.push(Box::new(v) as Box<dyn ToSql + 'static>);
        }

        sql.push(')');

        RawQuery::new(sql, params)
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
    /// Combine two filters with an AND operator
    /// ## Example
    /// ```rust
    /// User::select().filter(User::name.eq("John").and(User::age.gt(18))).exec(conn);
    /// ```
    /// This will generate the following SQL query:
    /// ```sql
    /// SELECT * FROM users WHERE users.name = ? AND users.age > ?;
    /// ```
    /// 
    /// ## Note
    /// This is not a beautiful way to write this query, so you should use '&' instead:
    /// ```rust
    /// User::select().filter(User::name.eq("John") & User::age.gt(18)).exec(conn);
    /// ```
    fn and<F1: ModelQueryFilter>(self, filter: F1) -> ModelQueryFilterAnd<Self, F1>
    where
        Self: Sized,
    {
        ModelQueryFilterAnd {
            filter0: self,
            filter1: filter,
        }
    }

    /// Combine two filters with an OR operator
    /// ## Example
    /// ```rust
    /// User::select().filter(User::name.eq("John").or(User::age.gt(18))).exec(conn);
    /// ```
    /// This will generate the following SQL query:
    /// ```sql
    /// SELECT * FROM users WHERE users.name = ? OR users.age > ?;
    /// ```
    /// 
    /// ## Note
    /// This is not a beautiful way to write this query, so you should use '|' instead:
    /// ```rust
    /// User::select().filter(User::name.eq("John") | User::age.gt(18)).exec(conn);
    /// ```
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
    ($op:ident ($fn:ident), $target:ident => $result:ident, $doc:expr) => {
        impl<T: ModelQueryFilter> $op<T> for $target {
            type Output = $result<Self, T>;

            #[doc = $doc]
            fn $fn(self, rhs: T) -> Self::Output {
                $result {
                    filter0: self,
                    filter1: rhs,
                }
            }
        }
    };
}

impl_op!(BitAnd (bitand), ColumnQueryFilter => ModelQueryFilterAnd, "Alternative to [ModelQueryFilterExt::and]");
impl_op!(BitOr (bitor), ColumnQueryFilter => ModelQueryFilterOr, "Alternative to [ModelQueryFilterExt::or]");

impl_op!(BitAnd (bitand), InQueryFilter => ModelQueryFilterAnd, "Alternative to [ModelQueryFilterExt::and]");
impl_op!(BitOr (bitor), InQueryFilter => ModelQueryFilterOr, "Alternative to [ModelQueryFilterExt::or]");

impl_op!(BitAnd (bitand), ColumnQueryFilterUnary => ModelQueryFilterAnd, "Alternative to [ModelQueryFilterExt::and]");
impl_op!(BitOr (bitor), ColumnQueryFilterUnary => ModelQueryFilterOr, "Alternative to [ModelQueryFilterExt::or]");


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