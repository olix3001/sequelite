use std::fmt::Debug;

use crate::{sql_types::{SqliteType, SqliteFlag}, IntoSqlite};

pub struct Column<'a> {
    name: &'a str,
    name_str: String,
    pub ty: SqliteType,
    flags: &'a [SqliteFlag],
    flags_vec: Vec<SqliteFlag>,

    default: Option<DefaultValue>,
}

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
            name_str: self.name_str.clone(),
            ty: self.ty,
            flags: self.flags,
            flags_vec: self.flags_vec.clone(),

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

        sql
    }
}

impl<'a> Column<'a> {
    pub fn new(name: String, ty: SqliteType, flags: Vec<SqliteFlag>, default: Option<Box<dyn IntoSqlite>>) -> Column<'a> {
        Column {
            name: "",
            name_str: name,
            ty,
            flags: &[],
            flags_vec: flags,

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

    pub fn name(&self) -> String {
        if self.name_str.is_empty() {
            self.name.to_string()
        } else {
            self.name_str.clone()
        }
    }

    pub fn has_flag(&self, flag: SqliteFlag) -> bool {
        self.flags().contains(&flag)
    }

    pub fn has_default(&self) -> bool {
        self.default.is_some()
    }

    pub fn same_default(&self, other: &Self) -> bool {
        match (&self.default, &other.default) {
            (Some(DefaultValue::Owned(a)), Some(DefaultValue::Owned(b))) => a.into_sqlite() == b.into_sqlite(),
            (Some(DefaultValue::Ref(a)), Some(DefaultValue::Ref(b))) => a.into_sqlite() == b.into_sqlite(),
            _ => false
        }
    }

    pub fn can_insert_null(&self) -> bool {
        !self.has_flag(SqliteFlag::NotNull) || self.has_flag(SqliteFlag::PrimaryKey) || self.has_default()
    }
}

impl Column<'static> {
    pub const fn new_const(name: &'static str, ty: SqliteType, flags: &'static [SqliteFlag], default: Option<&'static dyn IntoSqlite>) -> Column<'static> {
        let def = match default {
            Some(def) => Some(DefaultValue::Ref(def)),
            None => None
        };
        
        Column {
            name,
            name_str: String::new(),
            ty,
            flags,
            flags_vec: Vec::new(),

            default: def,
        }
    }
    
    pub const fn name_const(&self) -> &'static str {
        self.name
    }
}