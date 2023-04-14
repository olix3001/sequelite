use std::fmt::Debug;

use rusqlite::{types::FromSql, ToSql};

use crate::{IntoSqlite, prelude::{Executable, Connection, ColumnQueryFilterImpl}};

use super::{Model, query::ModelQuery, Column};

/// Represents a relation between two models.
/// 
/// ## Example use
/// ```rust
/// use sequelite::prelude::*;
/// 
/// #[derive(Debug, Model)]
/// struct User {
///     id: Option<i32>,
///     name: String,
/// }
/// 
/// #[derive(Debug, Model)]
/// struct Post {
///     id: Option<i32>,
///     title: String,
///     body: String,
/// 
///     author: Relation<User>
/// }
/// 
/// let mut conn = Connection::new_memory().unwrap();
/// conn.register::<User>().unwrap();
/// conn.register::<Post>().unwrap();
/// conn.migrate();
/// 
/// let user_id = User {
///     id: None,
///     name: "John Doe".to_string(),
/// }.insert(&conn).unwrap();
/// 
/// let post_id = Post {
///     id: None,
///     title: "Hello world!".to_string(),
///     body: "This is my first post!".to_string(),
///     author: Relation::id(user_id)
/// }.insert(&conn).unwrap();
/// 
/// let post = Post::select().filter(Post::author.ref_::<User>(user_id)).exec(&conn).unwrap().pop().unwrap();
/// assert_eq!(post_id, post.author.get_id());
/// ```
pub struct Relation<M> where M: Model {
    related_key: Option<i64>,
    related: Option<M>,

    marker: std::marker::PhantomData<M>
}

impl<M: Model> ToSql for Relation<M> {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(rusqlite::types::Value::Integer(self.related_key.unwrap_or(0))))
    }
}

impl<M: Model> Clone for Relation<M> {
    fn clone(&self) -> Self {
        Relation {
            related_key: self.related_key,
            related: None,

            marker: Default::default()
        }
    }
}

impl<M: Model> FromSql for Relation<M> {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Ok(Relation {
            related_key: value.as_i64().ok(),
            related: None,

            marker: Default::default()
        })
    }
}

impl<M> Default for Relation<M> where M: Model {
    fn default() -> Self {
        Self {
            related_key: None,
            related: None,

            marker: Default::default()
        }
    }
}

impl<M: Model + Debug> Debug for Relation<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.related.is_none() {
            f.debug_struct("UnfetchedRelation")
                .field("table", &M::table_name())
                .field("id", &self.related_key.unwrap())
                .finish()
        } else {
            f.debug_struct("Relation")
                .field("table", &M::table_name())
                .field("id", &self.related_key.unwrap())
                .field("model", &self.related.as_ref().unwrap())
                .finish()
        }
    }
}

impl<M: Model> Relation<M> {
    /// Create a new relation from an id in the related table.
    pub fn id(id: impl Into<i64>) -> Self {
        Relation {
            related_key: Some(id.into()),
            ..Default::default()
        }
    }

    /// Create a new relation from a model that is already in the database.
    pub fn model(model: &M) -> Self {
        Relation {
            related_key: Some(model.get_id()),
            ..Default::default()
        }
    }

    /// This function is used to fetch the related model from the database.
    /// It should not be called manually unless you know what you're doing.
    pub fn parse_from_row(row: &rusqlite::Row, offset: usize, idx: usize, counter: &mut usize, is_joined: bool) -> Self {
        if !is_joined {
            let related_key = row.get::<_, i64>(offset + idx);

            return Relation {
                related_key: Some(related_key.unwrap()),
                ..Default::default()
            }
        }


        let related = M::parse_row(row, offset + *counter, &Vec::new());
        *counter += M::count_columns();
        
        Relation {
            related_key: Some(related.get_id()),
            related: Some(related),

            marker: Default::default()
        }
    }

    /// Get the id of the related model
    pub fn get_id(&self) -> Option<i64> {
        self.related_key
    }

    /// Try to get the related model if it's already loaded
    pub fn try_get(&self) -> Option<&M> {
        self.related.as_ref()
    }

    /// Try to get the related model taking it out if it's already loaded
    pub fn try_take(&mut self) -> Option<M> {
        self.related.take()
    }

    /// Get the related model if it's already loaded, otherwise fetch it from the database
    pub fn get(&mut self, conn: &Connection) -> rusqlite::Result<&M> {
        if self.related.is_none() {
            self.fetch(conn)?;
        }

        Ok(self.related.as_ref().unwrap())
    }

    /// Get the related model taking it out if it's already loaded, otherwise fetch it from the database
    pub fn take(&mut self, conn: &Connection) -> rusqlite::Result<M> {
        if self.related.is_none() {
            self.fetch(conn)?;
        }

        Ok(self.related.take().unwrap())
    }

    /// Fetch the related model from the database
    pub fn fetch(&mut self, conn: &Connection) -> rusqlite::Result<&M> {
        let select_query = ModelQuery::<M>::select()
            .filter(M::id_column().eq(self.get_id()))
            .limit(1);

        if self.related.is_none() {
            self.related = Some(select_query.exec(conn).unwrap().into_iter().next().unwrap());
        }

        Ok(self.related.as_ref().unwrap())
    }

    /// Fetch the related model from the database and take it out
    pub fn fetch_once(&self, conn: &Connection) -> rusqlite::Result<M> {
        let select_query = ModelQuery::<M>::select()
            .filter(M::id_column().eq(self.get_id()))
            .limit(1);

        Ok(select_query.exec(conn).unwrap().into_iter().next().unwrap())
    }
}

/// Internally used by a column
#[derive(Debug, Clone, Copy)]
pub struct ColumnRelation<'a> {
    pub table: &'a str,
    pub column: &'a str,

    pub local_table: &'a str,

    pub foreign_key_column: &'a Column<'static>,
    pub local_key_column_name: &'a str,
    
    pub on_delete: ColumnRelationAction,
    pub on_update: ColumnRelationAction,
}

impl<'a> ColumnRelation<'a> {
    pub const fn new(table: &'a str, local_table: &'a str, column: &'a str, ref_col: &'static Column<'static>, local_col: &'a str) -> Self {
        ColumnRelation {
            table,
            column,
            local_table,
            foreign_key_column: ref_col,
            local_key_column_name: local_col,
            on_delete: ColumnRelationAction::Restrict,
            on_update: ColumnRelationAction::Restrict,
        }
    }

    pub const fn on_delete(mut self, action: ColumnRelationAction) -> Self {
        self.on_delete = action;
        self
    }

    pub const fn on_update(mut self, action: ColumnRelationAction) -> Self {
        self.on_update = action;
        self
    }
}

impl<'a> IntoSqlite for ColumnRelation<'a> {
    fn into_sqlite(&self) -> String {
        let mut sql = format!("REFERENCES {}({})", self.table, self.column);

        if self.on_delete != ColumnRelationAction::Restrict {
            sql.push_str(&format!(" ON DELETE {}", self.on_delete.into_sqlite()));
        }

        if self.on_update != ColumnRelationAction::Restrict {
            sql.push_str(&format!(" ON UPDATE {}", self.on_update.into_sqlite()));
        }

        sql
    }
}

/// The action that should be performed when a referenced row is deleted or updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnRelationAction {
    /// The default action. The database will not allow the deletion or update of the referenced row.
    Restrict,
    /// The database will delete or update the row from the table if that row is deleted or updated from the parent table.
    Cascade,
    /// The database will set the foreign key column or columns in the referencing row to NULL if that row is deleted or updated from the parent table.
    SetNull,
    /// The database will set the foreign key column or columns in the referencing row to the default value if that row is deleted or updated from the parent table.
    SetDefault,
    /// Just ignore and do nothing
    NoAction,
}

impl IntoSqlite for ColumnRelationAction {
    fn into_sqlite(&self) -> String {
        match self {
            ColumnRelationAction::Restrict => "RESTRICT".to_string(),
            ColumnRelationAction::Cascade => "CASCADE".to_string(),
            ColumnRelationAction::SetNull => "SET NULL".to_string(),
            ColumnRelationAction::SetDefault => "SET DEFAULT".to_string(),
            ColumnRelationAction::NoAction => "NO ACTION".to_string(),
        }
    }
}