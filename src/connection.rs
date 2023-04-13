use log::{info, debug, warn};
use rusqlite::{ToSql, types::ToSqlOutput};

use crate::{model::{Model, Column, migrator::{DbSchema, Migrator}}, IntoSqliteTy, sql_types::{SqliteFlag, SqliteType}};

pub struct Connection {
    pub connection: rusqlite::Connection,
    latest_schema: DbSchema<'static>
}

impl Connection {
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let connection = rusqlite::Connection::open(path)?;
        env_logger::init();
        Ok(Connection {
            connection,
            latest_schema: DbSchema::new()
        })
    }

    pub fn new_memory() -> Result<Self, rusqlite::Error> {
        let connection = rusqlite::Connection::open_in_memory()?;
        env_logger::init();
        Ok(Connection {
            connection,
            latest_schema: DbSchema::new()
        })
    }

    pub fn register<M: Model>(&mut self) -> Result<(), rusqlite::Error> {
        self.latest_schema.add_table::<M>();
        Ok(())
    }

    pub fn add_table<M: Model + IntoSqliteTy>(&self) -> Result<(), rusqlite::Error> {
        let sql = M::into_sqlite();
        self.connection.execute(&sql, [])?;
        Ok(())
    }

    pub fn drop_table<M: Model>(&self) -> Result<(), rusqlite::Error> {
        let sql = format!("DROP TABLE IF EXISTS {}", M::table_name());
        self.connection.execute(&sql, [])?;
        Ok(())
    }

    pub(crate) fn execute_no_params(&self, sql: &str) -> Result<(), rusqlite::Error> {
        self.connection.execute(sql, [])?;
        Ok(())
    }

    pub fn exec_raw(&self, sql: &str, params: &[&dyn ToSql]) -> Result<(), rusqlite::Error> {
        debug!(target: "query", "Executing raw query: \"{}\"", sql);
        self.connection.execute(sql, params)?;
        Ok(())
    }

    pub fn query_raw<F, T>(&self, sql: &str, params: &[&dyn ToSql], callback: F) -> Result<T, rusqlite::Error> where F: Fn(&rusqlite::Rows) -> T {
        debug!(target: "query", "Executing raw query: \"{}\"", sql);
        let mut stmt = self.connection.prepare(sql)?; 
        let rows = stmt.query(params)?;
        Ok(callback(&rows))
    }

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

    pub fn get_all_columns<'a>(&self, table: &str) -> Result<Vec<Column<'a>>, rusqlite::Error> {
        let mut stmt = self.connection.prepare(&format!("PRAGMA table_info({})", table))?;
        let mut rows = stmt.query([])?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            let ty: String = row.get(2)?;
            let not_null: bool = row.get(3)?;
            let pk: bool = row.get(5)?;

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
            // TODO: get default value
            let column = Column::new(name, ty.unwrap(), flags, None);
            columns.push(column);
        }
        Ok(columns)
    }

    pub fn migrate(&self) {
        info!(target: "migration", "Ensuring database is up to date...");
        Migrator::migrate(&self.latest_schema, &self)
    }

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

    // Yes I know that this could be more readable and that these generics are shit
    pub fn insert<I0: Insertable, I: IntoInsertable<Insertable = I0>>(&self, insertable: I) -> Result<(), rusqlite::Error> {
        let mut insertable = insertable.into_insertable();
        let raw_query = insertable.get_query();
        debug!(target: "query", "Executing query: {:?}", raw_query.sql);
        let params = raw_query.params.iter().map(|p| p.as_ref()).collect::<Vec<&dyn ToSql>>();
        let params = params.as_slice();
        self.connection.execute(&raw_query.sql, params)?;
        Ok(())
    }
}

pub trait Executable<T> {
    fn exec(self, conn: &Connection) -> Result<T, rusqlite::Error>;
}

pub trait Queryable<T> {
    fn get_query(&mut self) -> RawQuery;
    fn parse_result(&mut self, rows: rusqlite::Rows) -> T;
    fn should_execute(&self) -> bool {
        false
    }
}

pub trait IntoQueryable<T> {
    type Queryable: Queryable<T>;

    fn into_queryable(self) -> Self::Queryable;
}

pub trait Insertable {
    fn get_query(&mut self) -> RawQuery;
}

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