use std::fmt::Debug;

use crate::{sql_types::{SqliteType, SqliteFlag}, IntoSqlite, prelude::ColumnQueryFilterImpl};

use super::{relation::ColumnRelation, query::InQueryFilter, Model, ModelExt};

/// A column of a model.
/// This struct is quite big, so it is automatically implemented for every column in a struct that derives [Model](sequelite_macro::Model).
pub struct Column<'a> {
    name: &'a str,
    pub table_name: &'a str,
    name_str: String,
    pub ty: SqliteType,
    flags: &'a [SqliteFlag],
    flags_vec: Vec<SqliteFlag>,

    relation: Option<ColumnRelation<'a>>,

    default: Option<DefaultValue>,
}

/// A default value for a column.
/// This is used to implement the [default_value](sequelite_macro::default_value) attribute.
/// 
/// This is an enum that can either be a reference to a static value or a boxed value.
pub enum DefaultValue {
    Owned(Box<dyn IntoSqlite>),
    Ref(&'static dyn IntoSqlite)
}

impl IntoSqlite for DefaultValue {
    fn into_sqlite(&self) -> String {
        match self {
            DefaultValue::Owned(v) => v.into_sqlite(),
            DefaultValue::Ref(v) => v.into_sqlite()
        }
    }
}

impl PartialEq for Column<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name() && self.ty == other.ty && self.flags() == other.flags()
    }
}

impl Clone for Column<'_> {
    fn clone(&self) -> Self {
        Column {
            name: self.name,
            table_name: self.table_name,
            name_str: self.name_str.clone(),
            ty: self.ty,
            flags: self.flags,
            flags_vec: self.flags_vec.clone(),

            relation: self.relation.clone(),

            default: None,
        }
    }
}

impl Debug for Column<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Column")
            .field("name", &self.name())
            .field("type", &self.ty)
            .field("flags", &self.flags())
            .finish()
    }
}

impl<'a> IntoSqlite for Column<'a> {
    fn into_sqlite(&self) -> String {
        // Base
        let mut sql = format!("{} {}", self.name, self.ty.into_sqlite());
        // Flags
        for flag in self.flags.iter() {
            sql = format!("{} {}", sql, flag.into_sqlite());
        }
        // Default
        if let Some(def) = &self.default {
            sql = format!("{} DEFAULT {}", sql, def.into_sqlite());
        }

        // Foreign key
        if let Some(relation) = &self.relation {
            sql = format!("{}, FOREIGN KEY({}) {}", sql, self.name(), relation.into_sqlite());
        }

        sql
    }
}

impl<'a> Column<'a> {
    pub fn new(name: String, table_name: &'a str, ty: SqliteType, flags: Vec<SqliteFlag>, default: Option<Box<dyn IntoSqlite>>, relation: Option<ColumnRelation<'a>>) -> Column<'a> {
        Column {
            name: "",
            table_name,
            name_str: name,
            ty,
            flags: &[],
            flags_vec: flags,

            relation,

            default: default.map(|def| DefaultValue::Owned(def)),
        }
    }
    
    pub fn flags(&self) -> Vec<SqliteFlag> {
        if self.flags_vec.is_empty() {
            self.flags.to_vec()
        } else {
            self.flags_vec.clone()
        }
    }

    /// Returns the name of the column.
    /// 
    /// ## Example
    /// ```rust
    /// User::id.name() == "id"
    /// ```
    pub fn name(&self) -> String {
        if self.name_str.is_empty() {
            self.name.to_string()
        } else {
            self.name_str.clone()
        }
    }

    /// Check if the column has a specific flag.
    /// 
    /// ## Example
    /// ```rust
    /// User::id.has_flag(SqliteFlag::PrimaryKey) == true
    /// ```
    pub fn has_flag(&self, flag: SqliteFlag) -> bool {
        self.flags().contains(&flag)
    }

    /// Check if the column has a default value.
    pub fn has_default(&self) -> bool {
        self.default.is_some()
    }

    // For the future use.
    #[allow(dead_code)]
    pub(crate) fn same_default(&self, other: &Self) -> bool {
        match (&self.default, &other.default) {
            (Some(DefaultValue::Owned(a)), Some(DefaultValue::Owned(b))) => a.into_sqlite() == b.into_sqlite(),
            (Some(DefaultValue::Ref(a)), Some(DefaultValue::Ref(b))) => a.into_sqlite() == b.into_sqlite(),
            _ => false
        }
    }

    pub(crate) fn same_flags(&self, other: &Self) -> bool {
        for flag in self.flags() {
            if !other.has_flag(flag) {
                return false;
            }
        }
        true
    }

    pub fn can_insert_null(&self) -> bool {
        !self.has_flag(SqliteFlag::NotNull) || self.has_flag(SqliteFlag::PrimaryKey) || self.has_default()
    }

    /// Shorthand method for filtering by a relation.
    /// 
    /// # Expanded Example
    /// ```rust
    /// // Short form
    /// Post::select().filter(Post::author.ref_::<User>(1))
    /// 
    /// // Expanded form
    /// Post::select().filter(Post::author.in_(User::select().columns(&[User::id]).with_id(1).limit(1)))
    /// ```
    /// 
    /// # Panics
    /// Panics if the column is not a relation
    pub fn ref_<M: Model + ModelExt<M>>(self, id: i64) -> InQueryFilter where Self: ColumnQueryFilterImpl {
        match &self.relation.clone() {
            Some(relation) => {
                self.in_(
                    M::select().columns(&[relation.foreign_key_column.to_owned()]).with_id(id)
                )
            },
            None => panic!("Column {} is not a relation so you cannot use .id(...) on it", self.name())
        }
    }
}

impl Column<'static> {
    /// Creates a new column with static lifetime.
    /// This is used in the [Model](sequelite_macro::Model) macro.
    pub const fn new_const(name: &'static str, table_name: &'static str, ty: SqliteType, flags: &'static [SqliteFlag], default: Option<&'static dyn IntoSqlite>, relation: Option<ColumnRelation<'static>>) -> Column<'static> {
        let def = match default {
            Some(def) => Some(DefaultValue::Ref(def)),
            None => None
        };
        
        Column {
            name,
            table_name,
            name_str: String::new(),
            ty,
            flags,
            flags_vec: Vec::new(),

            relation,

            default: def,
        }
    }
    
    pub const fn name_const(&self) -> &'static str {
        self.name
    }

    pub const fn get_relation(&self) -> Option<ColumnRelation<'static>> {
        self.relation
    }
}