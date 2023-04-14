use log::{info, debug, warn};
use rusqlite::{ToSql, types::{ToSqlOutput, ValueRef}};

use crate::{model::{Model, Column, migrator::{DbSchema, Migrator}}, IntoSqliteTy, sql_types::{SqliteFlag, SqliteType}};

/// A connection to a SQLite database. This is the main entry point for interacting with the database.
/// 
/// ## Example
/// ```rs
/// let mut conn = sequelite::Connection::new("my_database.db").unwrap();
/// ```
pub struct Connection {
    pub connection: rusqlite::Connection,
    latest_schema: DbSchema<'static>
}

impl Connection {
    /// Creates a new connection to a SQLite database.
    /// 
    /// ## Arguments
    /// * `path` - The path to the database file.
    /// 
    /// ## Example
    /// ```rs
    /// let mut conn = Connection::new("my_database.db").unwrap();
    /// ```
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let connection = rusqlite::Connection::open(path)?;
        let _ = env_logger::try_init();
        Ok(Connection {
            connection,
            latest_schema: DbSchema::new()
        })
    }

    /// Creates a new connection to a transient SQLite database in memory.
    /// 
    /// ## Example
    /// ```rs
    /// let mut conn = Connection::new_memory().unwrap();
    /// ```
    pub fn new_memory() -> Result<Self, rusqlite::Error> {
        let connection = rusqlite::Connection::open_in_memory()?;
        let _ = env_logger::try_init();
        Ok(Connection {
            connection,
            latest_schema: DbSchema::new()
        })
    }

    /// Registers a model with the connection.
    /// ## What does this do?
    /// This method will add the model to the list of watched models.
    /// Models are watched by the migrator to ensure that the database schema is up to date every time `connection.migrate()` is called.
    /// 
    /// ## Example
    /// ```rs
    /// #[derive(Model)]
    /// struct User {
    ///     id: Option<i32>,
    ///     name: String
    /// }
    /// 
    /// let mut conn = Connection::new_memory().unwrap();
    /// conn.register::<User>().unwrap();
    /// conn.migrate();
    /// ```
    pub fn register<M: Model>(&mut self) -> Result<(), rusqlite::Error> {
        self.latest_schema.add_table::<M>();
        Ok(())
    }

    /// Execute query which creates a table if it doesn't exist.
    pub fn add_table<M: Model + IntoSqliteTy>(&self) -> Result<(), rusqlite::Error> {
        let sql = M::into_sqlite();
        self.connection.execute(&sql, [])?;
        Ok(())
    }

    /// Execute query which drops a table if it exists.
    pub fn drop_table<M: Model>(&self) -> Result<(), rusqlite::Error> {
        let sql = format!("DROP TABLE IF EXISTS {}", M::table_name());
        self.connection.execute(&sql, [])?;
        Ok(())
    }

    pub(crate) fn execute_no_params(&self, sql: &str) -> Result<(), rusqlite::Error> {
        debug!(target: "query_internal", "Executing query: \"{}\"", sql);
        self.connection.execute(sql, [])?;
        Ok(())
    }

    /// Execute a raw query on the database.
    /// 
    /// **note:** This method does not return any data. Use `query_raw` if you want to return data.
    /// 
    /// ## Arguments
    /// * `sql` - The SQL query to execute.
    /// * `params` - The parameters to pass to the query.
    /// 
    /// ## Returns
    /// The number of rows affected by the query.
    /// 
    /// ## Example
    /// ```rs
    /// let mut conn = Connection::new_memory().unwrap();
    /// conn.exec_raw("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)", &[]).unwrap();
    /// ```
    pub fn exec_raw(&self, sql: &str, params: &[&dyn ToSql]) -> Result<usize, rusqlite::Error> {
        debug!(target: "query", "Executing raw query: \"{}\"", sql);
        let n = self.connection.execute(sql, params)?;
        Ok(n)
    }

    /// Execute a raw query on the database.
    /// 
    /// **note:** This may not work for queries like `CREATE TABLE` or `UPDATE`. Use `exec_raw` if you don't want to return data.
    /// 
    /// ## Arguments
    /// * `sql` - The SQL query to execute.
    /// * `params` - The parameters to pass to the query.
    /// * `callback` - A callback function which will be called with the result of the query as a `rusqlite::Rows` object.
    /// 
    /// ## Returns
    /// The result of the callback function.
    /// 
    /// ## Example
    /// ```rs
    /// let mut conn = Connection::new_memory().unwrap();
    /// let users = conn.query_raw("SELECT * FROM users", &[], |rows| {
    ///    let mut users = Vec::new();
    ///    for row in rows {
    ///        users.push(
    ///            User {
    ///                id: row.get(0).unwrap(),
    ///                name: row.get(1).unwrap()
    ///            }
    ///        );
    ///    }
    ///    users
    /// }).unwrap();
    /// ```
    pub fn query_raw<F, T>(&self, sql: &str, params: &[&dyn ToSql], callback: F) -> Result<T, rusqlite::Error> where F: Fn(&rusqlite::Rows) -> T {
        debug!(target: "query", "Executing raw query: \"{}\"", sql);
        let mut stmt = self.connection.prepare(sql)?; 
        let rows = stmt.query(params)?;
        Ok(callback(&rows))
    }

    /// Get the names of all tables in the database.
    /// 
    /// **WARNING:** This should not be used outside of the migrator. It is not guaranteed to work in the future.
    pub fn get_all_tables(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self.connection.prepare("SELECT name FROM sqlite_master WHERE type='table'")?;
        let mut rows = stmt.query([])?;
        let mut tables = Vec::new();
        while let Some(row) = rows.next()? {
            let rn = row.get(0)?;
            if rn != "sqlite_sequence" {
                tables.push(rn);
            }
        }
        Ok(tables)
    }

    /// Get all columns in a table.
    /// 
    /// **WARNING:** This should not be used outside of the migrator. It is not guaranteed to work in the future.
    pub fn get_all_columns<'a>(&self, table: &str) -> Result<Vec<Column<'a>>, rusqlite::Error> {
        let mut stmt = self.connection.prepare(&format!("PRAGMA table_info({})", table))?;
        let mut rows = stmt.query([])?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            let ty: String = row.get(2)?;
            let not_null: bool = row.get(3)?;
            let pk: bool = row.get(5)?;
            // TODO: Default value
            let _default_value: Option<ValueRef> = row.get_ref(4).ok();

            let mut flags = Vec::new();
            if not_null {
                flags.push(SqliteFlag::NotNull);
            }
            if pk {
                flags.push(SqliteFlag::PrimaryKey);
            }

            // Check for autoincrement
            let mut stmt = self.connection.prepare(&format!("SELECT 'is-autoincrement' FROM sqlite_master WHERE tbl_name='{}' AND sql LIKE '%AUTOINCREMENT%'", table))?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let is_autoincrement: String = row.get(0)?;
                if is_autoincrement == "is-autoincrement" && pk {
                    flags.push(SqliteFlag::AutoIncrement);
                }
            }

            let ty = SqliteType::from_str(&ty);
            let column = Column::new(name, "", ty.unwrap(), flags, None, None);
            columns.push(column);
        }
        Ok(columns)
    }

    /// Migrates the database to the latest schema.
    /// 
    /// This will create new tables, add new columns, remove old columns, modify tables, etc.
    /// 
    /// ## Example:
    /// ```rs
    /// use sequelite::prelude::*;
    /// 
    /// #[derive(Model)]
    /// struct User {
    ///     id: Option<i32>,
    ///     name: String
    /// }
    /// 
    /// let mut conn = Connection::new_memory().unwrap();
    /// conn.register::<User>();
    /// conn.migrate();
    /// ```
    /// 
    /// ## Notes:
    /// You can enable `RUST_LOG=debug` to see the migration queries.
    pub fn migrate(&self) {
        info!(target: "migration", "Ensuring database is up to date...");
        Migrator::migrate(&self.latest_schema, &self)
    }

    /// Execute a query on the database.
    /// 
    /// ## Arguments
    /// * `query` - The query to execute.
    /// 
    /// ## Returns
    /// The number of rows affected.
    /// 
    /// ## Notes
    /// You most likely want to use `query` instead of this function.
    pub fn exec<Q0: Queryable<()>, Q: IntoQueryable<(), Queryable = Q0>>(&self, query: Q) -> Result<usize, rusqlite::Error> {
        let mut query = query.into_queryable();
        if !query.should_execute() {
            warn!(target: "query", "Statement should be queried, but is executed. Consider using query() instead.");
        }

        let raw_query = query.get_query();
        debug!(target: "query", "Executing query: {:?}", raw_query.sql);
        let params = raw_query.params.iter().map(|p| p.as_ref()).collect::<Vec<&dyn ToSql>>();
        let params = params.as_slice();
        self.connection.execute(&raw_query.sql, params)
    }

    /// Execute a query on the database.
    /// 
    /// ## Arguments
    /// * `query` - The query to execute.
    /// 
    /// ## Returns
    /// The result of the query.
    /// 
    /// ## Example
    /// ```rs
    /// use sequelite::prelude::*;
    /// 
    /// #[derive(Model)]
    /// struct User {
    ///     id: Option<i32>,
    ///     name: String
    /// }
    /// 
    /// let mut conn = Connection::new_memory().unwrap();
    /// conn.register::<User>();
    /// conn.migrate();
    /// 
    /// let user_id = User {
    ///     id: None,
    ///     name: "John".to_string()
    /// }.insert(&conn).unwrap();
    /// 
    /// let user_query = User::select().with_id(user_id);
    /// 
    /// let user = conn.query(user_query).unwrap();
    /// assert_eq!(user.name, "John");
    /// ```
    /// 
    /// ## Notes
    /// It is recommended to use `query.exec(&conn)` as it automatically checks if the query should be executed or queried.
    pub fn query<T, Q0: Queryable<T>, Q: IntoQueryable<T, Queryable = Q0>>(&self, query: Q) -> Result<T, rusqlite::Error> {
        // Hi, I'm just a wall of random code :>
        let mut query = query.into_queryable();
        if query.should_execute() {
            warn!(target: "query", "Statement should be executed, but is queried. Consider using exec() instead.");
        }

        let raw_query = query.get_query();
        debug!(target: "query", "Executing query: {:?}", raw_query.sql);
        let params = raw_query.params.iter().map(|p| p.as_ref()).collect::<Vec<&dyn ToSql>>();
        let params = params.as_slice();
        let mut stmt = self.connection.prepare(&raw_query.sql)?;
        let rows = stmt.query(params)?;
        let result = query.parse_result(rows);
        Ok(result)
    }

    /// Insert data into the database.
    /// 
    /// ## Arguments
    /// * `insertable` - The data to insert. This can be a struct, vector or slice.
    /// 
    /// ## Returns
    /// The id of the inserted row. (If there are multiple rows, the id of the last row is returned.)
    /// 
    /// ## Example
    /// ```rs
    /// use sequelite::prelude::*;
    /// 
    /// #[derive(Model)]
    /// struct User {
    ///     id: Option<i32>,
    ///     name: String
    /// }
    /// 
    /// let mut conn = Connection::new_memory().unwrap();
    /// conn.register::<User>();
    /// conn.migrate();
    /// 
    /// conn.insert(User {
    ///     id: None, // Id will be auto generated by the database
    ///     name: "John".to_string()
    /// }).unwrap();
    /// ```
    /// 
    /// ## Note
    /// There is an an easier way to insert data:
    /// ```rs
    /// // Same as above
    /// let user_id = User {
    ///     id: None,
    ///     name: "John".to_string()
    /// }.insert(&conn).unwrap();
    /// ```
    // Yes I know that this could be more readable and that these generics are shit
    pub fn insert<I0: Insertable, I: IntoInsertable<Insertable = I0>>(&self, insertable: I) -> Result<i64, rusqlite::Error> {
        let mut insertable = insertable.into_insertable();
        let raw_query = insertable.get_query();
        debug!(target: "query", "Executing query: {:?}", raw_query.sql);
        let params = raw_query.params.iter().map(|p| p.as_ref()).collect::<Vec<&dyn ToSql>>();
        let params = params.as_slice();
        self.connection.execute(&raw_query.sql, params)?;
        
        // Get last row id
        let last_row_id = self.connection.last_insert_rowid();
        Ok(last_row_id)
    }
}

/// It is implemented for everything that has `.exec(&conn)` method.
pub trait Executable<T> {
    fn exec(self, conn: &Connection) -> Result<T, rusqlite::Error>;
}

/// Trait that represents everything that can be used as a query in `connection.query(...)`
pub trait Queryable<T> {
    fn get_query(&mut self) -> RawQuery;
    fn parse_result(&mut self, rows: rusqlite::Rows) -> T;
    fn should_execute(&self) -> bool {
        false
    }
}

/// Trait that should be implemented for everything that can be made into a query (including queries themselves).
pub trait IntoQueryable<T> {
    type Queryable: Queryable<T>;

    fn into_queryable(self) -> Self::Queryable;
}

/// Trait that represents everything that can be inserted in `connection.insert(...)`
pub trait Insertable {
    fn get_query(&mut self) -> RawQuery;
}

/// Trait that should be implemented for everything that can be made into an insertable (including insertables themselves).
pub trait IntoInsertable {
    type Insertable: Insertable;

    fn into_insertable(&self) -> Self::Insertable;
}

impl<'a, T, Q0: Queryable<T>> IntoQueryable<T> for Q0 {
    type Queryable = Q0;

    fn into_queryable(self) -> Self::Queryable {
        self
    }
}

/// A raw query that can be executed on a database.
/// This is used internally by sequelite. You should not need to use this.
pub struct RawQuery {
    pub sql: String,
    pub params: Vec<Box<dyn ToSql>>
}

impl RawQuery {
    pub fn new(sql: String, params: Vec<Box<dyn ToSql>>) -> Self {
        RawQuery {
            sql,
            params
        }
    }

    pub fn move_clone(&mut self) -> Self {
        let sql = std::mem::replace(&mut self.sql, String::new());
        let params = std::mem::replace(&mut self.params, Vec::new());
        RawQuery {
            sql,
            params
        }
    }

    /// Useful for debugging (and only for debugging)
    pub fn substitute_params(&self) -> String {
        let mut sql = self.sql.clone();
        for param in &self.params {
            let param = param.to_sql().unwrap();
            sql = sql.replace("?", &value_to_string(&param));
        }
        sql
    }
}

fn value_to_string(value: &dyn ToSql) -> String {
    let value = value.to_sql().unwrap();
    if let ToSqlOutput::Owned(ref value) = value {
        match value {
            rusqlite::types::Value::Integer(i) => i.to_string(),
            rusqlite::types::Value::Real(f) => f.to_string(),
            rusqlite::types::Value::Text(s) => s.to_string(),
            rusqlite::types::Value::Blob(b) => String::from_utf8(b.to_vec()).unwrap(),
            rusqlite::types::Value::Null => String::new()
        }
    } else {
        panic!("Cannot convert value to string");
    }
}