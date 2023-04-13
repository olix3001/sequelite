use sequelite::prelude::*;

#[derive(Debug, Model)]
struct User {
    id: Option<i32>,
    name: String,
}

fn main() {
    // Create a new in-memory database
    let mut conn = Connection::new_memory().unwrap();

    // Migrate the database if needed
    conn.register::<User>().unwrap();
    conn.migrate();

    // Create 10 users
    for i in 0..10 {
        User {
            id: None,
            name: format!("User {}", i)
        }.insert(&conn).unwrap();
    }

    // Update all users with id >= 5 to have the name "John Doe"
    User::update()
        .filter(User::id.ge(5))
        .set(User::name, "John Doe")
        .exec(&conn).unwrap();

    // Delete all users with id < 2 or id > 8
    User::delete()
        .filter(User::id.lt(2) | User::id.gt(8))
        .exec(&conn).unwrap();

    // Print all users
    println!("{:?}", User::select().exec(&conn).unwrap());
}
