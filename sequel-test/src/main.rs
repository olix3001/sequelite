use sequelite::model::relation::Relation;
use sequelite::prelude::*;

#[derive(Debug, Model, Default)]
struct User {
    id: Option<i32>,
    name: String,
}

#[derive(Debug, Model, Default)]
struct Post {
    id: Option<i32>,
    title: String,
    body: String,
    author: Relation<User>,
}

fn main() {
    // Create a new in-memory database
    let mut conn = Connection::new_memory().unwrap();

    // Migrate the database if needed
    conn.register::<User>().unwrap();
    conn.register::<Post>().unwrap();
    conn.migrate();

    // Create a new user
    let user_id = User {
        id: None,
        name: "John Doe".to_string(),
    }.insert(&conn).unwrap();

    // Create a new post
    conn.insert(&[
        Post {
            id: None,
            title: "Hello world!".to_string(),
            body: "This is my first post!".to_string(),
            author: Relation::id(user_id)
        },
        Post {
            id: None,
            title: "Hello sequelite!".to_string(),
            body: "This is my second post!".to_string(),
            author: Relation::id(user_id)
        },
    ]).unwrap();

    // Get posts by user
    let posts_query = Post::select()
        .filter(Post::author.ref_::<User>(user_id));

    println!("{:?}", posts_query.exec(&conn));
}
