use sequelite::prelude::*;
use sequelite::chrono;

#[derive(Debug, Model, Default)]
struct User {
    id: Option<i32>,
    name: String,
    nickname: Option<String>,

    #[default_value(&NowTime)]
    created_at: Option<chrono::NaiveDateTime>,
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
            name: format!("User {}", i),
            nickname: if i % 2 == 0 { Some(format!("Cool nickname {}", i)) } else { None },
            created_at: None,
            ..Default::default()
        }.insert(&conn).unwrap();
    }

    // Update all users with id >= 5 to have the name "John Doe"
    User::update()
        .filter(User::id.ge(5))
        .set(User::name, "John Doe")
        .exec(&conn).unwrap();

    // Delete all users without a nickname
    User::delete()
        .filter(User::nickname.is_null())
        .exec(&conn).unwrap();

    // Select all users
    let users = User::select().exec(&conn).unwrap();
    users.iter().for_each(|user| println!("{:?}", user));
}
