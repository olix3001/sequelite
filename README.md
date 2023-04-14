# Sequelite :rocket:

Sequelite is a simple, lightweight, and fast SQLite ORM for rust.

It is built on top of [rusqlite](https://crates.io/crates/rusqlite)

## Features

-   [x] Simple and easy to use
-   [x] Lightweight
-   [x] Fast
-   [x] Automatic schema migration

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
sequelite = "0.2"
```

You can find the documentation [here](https://docs.rs/sequelite)

## Example

```rust
use sequelite::{Database, Model, Table};

#[derive(Debug, Model)]
struct User {
    id: Option<i32>,
    #[default_value(&"Unknown name")]
    name: Option<String>,
    age: i32,
}

fn main() {
    // Create new database connection
    let mut conn = Connection::new("example.db").unwrap();

    // Ensure database schema is up to date
    conn.register::<User>().unwrap();
    conn.migrate();

    // Create a new users
    conn.insert(&[
        User { id: None, name: Some("John".to_string()), age: 20 },
        User { id: None, name: Some("Jane".to_string()), age: 21 },
    ]);

    // Get all users whose name is "John"
    let users = User::select()
        .filter(User::name.eq("John"))
        .exec(&conn).unwrap();

    // Print all users
    println!("{:#?}", users);
}
```
