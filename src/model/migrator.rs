use std::collections::HashMap;
use log::{warn, debug};

use crate::{connection::Connection, IntoSqlite};

use super::{Model, column::Column};

/// Migrator ensures that the database is up to date with the latest schema.
pub struct Migrator;

impl Migrator {
    /// Migrate the database to the latest schema.
    pub fn migrate(latest_schema: &DbSchema, connection: &Connection) {
        // Compare the latest schema with the current schema updating the database as needed.
        Self::migrate_models(latest_schema, connection);
    }

    #[allow(unreachable_code)]
    pub fn migrate_models(latest_schema: &DbSchema, connection: &Connection) {
        // Iterate over the tables in database and compare them to the latest schema.
        // If the table is not in the latest schema, drop it.
        // If the table is in the latest schema, compare the columns.
        // If the column is not in the database, add it.
        
        let tables = connection.get_all_tables().unwrap();

        for table in tables.iter() {
            if let Some(_) = latest_schema.tables.get(&table.clone()) {
                // The table is in the latest schema, compare the columns.
                let columns = connection.get_all_columns(&table).unwrap();
                
                // Remove columns that are not in the latest schema.
                for column in columns.iter() {
                    if latest_schema.tables.get(&table.clone()).unwrap().iter().find(|c| c.name() == column.name()).is_none() {
                        // The column is not in the latest schema, drop it.
                        // safety note: this is safe because the column name is checked against the latest schema.
                        connection.execute_no_params(&format!(
                            "ALTER TABLE {} DROP COLUMN {};",
                            table, column.name()
                        )).unwrap();

                        warn!(target: "migration", "Dropped column {} from table {}.", column.name(), table);
                    }
                }

                for latest_column in latest_schema.tables.get(&table.clone()).unwrap().iter() {
                    // Change existing columns.
                    if columns.iter().find(|c| c.name() == latest_column.name()).is_none() {
                        // The column is not in the latest schema, add it without modifying the data.
                        // safety note: this is safe because the column name is checked against the latest schema.
                        connection.execute_no_params(&format!(
                            "ALTER TABLE {} ADD COLUMN {};",
                            table, latest_column.into_sqlite()
                        )).unwrap();

                        warn!(target: "migration", "Added column {} to table {} without migrating data.", latest_column.name(), table);
                    }
                }
                let columns = connection.get_all_columns(&table).unwrap();

                for latest_column in latest_schema.tables.get(&table.clone()).unwrap().iter() {
                    let column = columns.iter().find(|c| c.name() == latest_column.name()).unwrap();

                    // The column is in the latest schema, compare the types.
                    if column.ty != latest_column.ty || column.flags() != latest_column.flags() || column.same_default(latest_column) {
                        // The column type is not the same, use alter table to change it.
                        // safety note: this is safe because the column name is checked against the latest schema.
                        replace_table_full(connection, table, latest_schema.tables.get(&table.clone()).unwrap());

                        warn!(target: "migration", "Migrated whole table while altering column {} in table {} from '{}' to '{}'.", column.name(), table, column.ty.into_sqlite(), latest_column.ty.into_sqlite());
                        break; // The table has been replaced, no need to continue.
                    }
                }

            } else {
                // The table is not in the latest schema, drop it.
                connection.execute_no_params(&format!("DROP TABLE IF EXISTS {}", table)).unwrap();

                warn!(target: "migration", "Dropped table {}.", table);
            }
        }

        // Create any tables that are in the latest schema but not in the database.
        for (table, columns) in latest_schema.tables.iter() {
            if !tables.contains(&table) {
                // The table is not in the database, create it.
                let mut sql = format!("CREATE TABLE {} (", table);
                for column in columns.iter() {
                    sql.push_str(&format!("{},", column.into_sqlite()));
                }
                sql.pop();
                sql.push(')');
                connection.execute_no_params(&sql).unwrap();

                debug!(target: "query_internal", "Created table using: {}", sql);

                warn!(target: "migration", "Created table {} as it has not been found in current database.", table);
            }
        }
    }
}

fn replace_table_full(connection: &Connection, table: &str, columns: &[Column]) {
    let mut sql = format!("CREATE TABLE temp_{}_new (", table);
    for column in columns.iter() {
        sql.push_str(&format!("{},", column.into_sqlite()));
    }
    sql.pop();
    sql.push(')');
    connection.execute_no_params(&sql).unwrap();

    // Copy the data from the old table to the new table.
    connection.execute_no_params(&format!(
        "INSERT INTO temp_{}_new SELECT * FROM {};",
        table, table
    )).unwrap();

    // Drop the old table.
    connection.execute_no_params(&format!("DROP TABLE IF EXISTS {}", table)).unwrap();

    // Rename the new table to the old table.
    connection.execute_no_params(&format!(
        "ALTER TABLE temp_{}_new RENAME TO {};",
        table, table
    )).unwrap();
}

pub struct DbSchema<'a> {
    // Name -> Fields
    pub tables: HashMap<String, &'a [Column<'a>]>
}

impl DbSchema<'_> {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new()
        }
    }

    pub fn add_table<M: Model>(&mut self) {
        self.tables.insert(M::table_name().to_string(), M::columns());
    }
}