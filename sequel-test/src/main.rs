use sequelite::{prelude::*, connection::{Executable}};

#[derive(Debug, Model)]
struct User {
    id: Option<i32>,
    name: String
}

fn main() {
    let mut conn = Connection::new_memory().unwrap();
    conn.register::<User>().unwrap();
    conn.migrate();

    User {
        id: None,
        name: "John".to_string(),
    }.insert(&conn).unwrap();

    let update_query = User::update()
        .set(User::name, "John Doe");

    update_query.exec(&conn).unwrap();

    println!("{:?}", User::select().exec(&conn).unwrap());
}
