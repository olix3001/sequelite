#![allow(clippy::wrong_self_convention)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::ptr_arg)]

//! A library for creating SQLite models.
//! 
//! ## Example
//! ```rs
//! use sequelite::prelude::*;
//! 
//! #[derive(Model)]
//! struct User {
//!     id: Option<i32>,
//!     name: String,
//! }
//! 
//! #[derive(Model)]
//! struct Post {
//!     id: Option<i32>,
//!     title: String,
//!     body: String,
//!     author: Relation<User>
//! }
//! 
//! let mut conn = Connection::new_memory().unwrap();
//! conn.register::<User>().unwrap();
//! conn.register::<Post>().unwrap();
//! conn.migrate();
//! 
//! let user_id = User {
//!     id: None,
//!     name: "John Doe".to_string(),
//! }.insert(&conn).unwrap();
//! 
//! conn.insert(&[
//!     Post {
//!         id: None,
//!         title: "Hello world!".to_string(),
//!         body: "This is my first post!".to_string(),
//!         author: Relation::id(user_id)
//!     },
//!     Post {
//!         id: None,
//!         title: "Hello sequelite!".to_string(),
//!         body: "This is my second post!".to_string(),
//!         author: Relation::id(user_id)
//!     }
//! ]).unwrap();
//! 
//! let posts_query = Post::select()
//!     .filter(Post::author.ref_::<User>(user_id));
//! 
//! println!("{:?}", posts_query.exec(&conn).unwrap());
//! ```

pub mod model;
pub mod sql_types;
pub mod connection;

/// A prelude for users of the library.
pub mod prelude {
    pub use crate::model::{Model, Column, ModelExt, SqliteRows, SqliteToSql,
        query::{ColumnQueryFilterImpl, ModelQueryFilterExt},
        relation::Relation
    };

    pub use crate::connection::Connection;
    pub use crate::connection::Executable;

    pub use sequelite_macro::Model;

    pub use rusqlite::Error as SqliteError;

    pub use crate::sql_types::NowTime;
}

pub extern crate rusqlite;
pub extern crate chrono;

/// A trait for converting a value to a SQLite value.
/// 
/// This is useful for making custom types that can be used as default value for columns.
/// 
/// ## Note
/// If you want to use a custom type as a column type, you need to implement the rusqlite's `ToSql` trait.
pub trait IntoSqlite {
    fn into_sqlite(&self) -> String;
}

pub trait IntoSqliteTy {
    fn into_sqlite() -> String;
}

#[cfg(test)]
mod tests {
    use crate as sequelite;
    use sequelite::prelude::*;

    #[derive(Debug, Model)]
    #[table_name = "test"]
    struct TestModel {
        id: Option<i32>,
        name: String,
        #[default_value(&0)]
        age: i32,

        even: Option<bool>
    }

    #[test]
    fn migrate_create_table() {
        let mut conn = Connection::new_memory().unwrap();
        conn.register::<TestModel>().unwrap();
        conn.migrate();

        let exists = conn.get_all_tables().unwrap().iter().any(|table| table == "test");
        assert!(exists);
    }

    #[test]
    fn crud() {
        let mut conn = Connection::new_memory().unwrap();
        conn.register::<TestModel>().unwrap();
        conn.migrate();


        // Create 10 users with random names (C)
        for i in 0..10 {
            TestModel {
                id: None,
                name: format!("User {}", i),
                age: (i*3)>>2/2%35,
                even: Some(i % 2 == 0)
            }.insert(&conn).unwrap();
        }

        // Create user without even (C)
        TestModel {
            id: None,
            name: "User 10".to_string(),
            age: 40,
            even: None
        }.insert(&conn).unwrap();

        // Update all users with id >= 5 to have the name "John Doe" (U)
        TestModel::update()
            .filter(TestModel::id.ge(5))
            .set(TestModel::name, "John Doe")
            .exec(&conn).unwrap();

        // Delete all users who are odd or unknown (D)
        TestModel::delete()
            .filter(TestModel::even.eq(false) | TestModel::even.is_null())
            .exec(&conn).unwrap();

        // Select all users (R)
        let users = TestModel::select().exec(&conn).unwrap();
        
        // Expect 5 users
        assert_eq!(users.len(), 5);

        // Check if all users are even
        users.iter().for_each(|user| assert!(user.even.unwrap()));

        // Check if all users with id >= 5 have the name "John Doe"
        users.iter().for_each(|user| {
            if user.id.unwrap() >= 5 {
                assert_eq!(user.name, "John Doe");
            }
        });

        // Check if all users with id < 5 have the name "User <id-1>"
        users.iter().for_each(|user| {
            if user.id.unwrap() < 5 {
                assert_eq!(user.name, format!("User {}", user.id.unwrap() - 1));
            }
        });

        // Delete oldest user
        TestModel::delete()
            .filter(
                TestModel::id.in_(
                    TestModel::select()
                        .columns(&[TestModel::id])
                        .order_by(TestModel::age.desc())
                        .limit(1)
                )
            )
            .exec(&conn).unwrap();
        
        // Check if oldest user is deleted (Count :p)
        let users40 = TestModel::count()
            .filter(TestModel::age.eq(40))
            .exec(&conn).unwrap();

        assert_eq!(users40, 0);

        // Remove all users where id is 0, 1, 2, 3, 4, 5, 6 or 7
        TestModel::delete()
            .filter(TestModel::id.in_(&[0, 1, 2, 3, 4, 5, 6, 7]))
            .exec(&conn).unwrap();

        // Check if all users are deleted
        println!("{:?}", TestModel::select().exec(&conn).unwrap());
        assert!(TestModel::count().exec(&conn).unwrap() == 0);
    }

    #[derive(Model)]
    #[table_name = "test"]
    struct MigrationTest0 {
        id: Option<i32>,
        name: String,
        money: Option<f64>,
        even: Option<bool>
    }

    #[derive(Model)]
    #[table_name = "test"]
    struct MigrationTest1 {
        id: Option<i32>,
        name: String,
        #[default_value(&0.0)]
        money: f64,
        #[default_value(&false)]
        even: bool,
    }

    #[test]
    fn migrate_basic() {
        // Check if basic migration works (Add column, Remove column)

        // Create table with TestModel
        let mut conn = Connection::new_memory().unwrap();
        conn.register::<TestModel>().unwrap();
        conn.migrate();

        // Insert 10 users
        for i in 0..10 {
            TestModel {
                id: None,
                name: format!("User {}", i),
                age: (i*3)>>2/2%35,
                even: Some(i % 2 == 0)
            }.insert(&conn).unwrap();
        }

        // Check if all users are inserted
        assert_eq!(TestModel::count().exec(&conn).unwrap(), 10);

        // Migrate to MigrationTest0
        conn.register::<MigrationTest0>().unwrap();
        conn.migrate();

        // Check if all users are still there
        assert_eq!(TestModel::count().exec(&conn).unwrap(), 10);
    }

    #[test]
    fn migrate_full() {
        // Check if full migration works (Add/Remove column, Change column type, Change column default value)

        // Create table with TestModel
        let mut conn = Connection::new_memory().unwrap();
        conn.register::<TestModel>().unwrap();
        conn.migrate();

        // Insert 10 users
        for i in 0..10 {
            TestModel {
                id: None,
                name: format!("User {}", i),
                age: (i*3)>>2/2%35,
                even: Some(i % 2 == 0)
            }.insert(&conn).unwrap();
        }

        // Check if all users are inserted
        assert_eq!(TestModel::count().exec(&conn).unwrap(), 10);

        // Migrate to MigrationTest1
        conn.register::<MigrationTest1>().unwrap();
        conn.migrate();

        // Check if all users are still there
        assert_eq!(TestModel::count().exec(&conn).unwrap(), 10);
    }

}