use proc_macro::TokenStream;
use quote::quote;

/// A macro for deriving the [Model](sequelite::model::Model) trait.
/// 
/// ## Attributes
/// * #\[table_name = "name"] - Custom table name. If not specified, the table name will be the lowercase of the struct name + 's'.
/// * #\[default_value(value)] - Default value for the column. If not specified, the default value will be NULL.
/// 
/// ## Example
/// ```rust
/// use sequelite::prelude::*;
/// 
/// #[derive(Model)]
/// struct User {
///     id: Option<i32>, // field named "id" will be primary key
///     name: String,
/// }
/// ```
#[proc_macro_derive(Model, attributes(default_value, table_name))]
pub fn model_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();

    // Get name from attribute table = "name" and otherwise use lowercase of struct name
    let name = ast.ident;

    // Iterate over all the fields of the struct
    let fields = match ast.data {
        syn::Data::Struct(ref data) => &data.fields,
        _ => panic!("Only structs are supported"),
    };

    let mut field_names = Vec::new();

    let mut column_value_getters = Vec::new();
    let mut column_value_setters = Vec::new();

    let mut id_column = quote!();
    let mut id_column_const = quote!();

    let fields_num = fields.len();

    // Generate const for each field
    let field_consts = fields.iter().enumerate().map(|(i, field)| {
        let field_name = &field.ident;
        field_names.push(field_name.clone().unwrap());
        let field_type = &field.ty;

        // Check if the field is an Option<T>
        let field_option = is_option(field_type);

        // Create vector of flags
        let mut flags = Vec::new();

        // Add NOT NULL flag if the field is not an Option<T>

        // If field name is ID, add PRIMARY KEY, AUTOINCREMENT and NOT NULL flags
        if let Some(ident) = field_name {
            if ident.to_string() == "id" {
                flags.push(quote!(sequelite::sql_types::SqliteFlag::PrimaryKey));
                flags.push(quote!(sequelite::sql_types::SqliteFlag::AutoIncrement));
                flags.push(quote!(sequelite::sql_types::SqliteFlag::NotNull));

                id_column = quote!(self.#ident);
                id_column_const = quote!(Self::#ident);
            } else if !field_option {
                flags.push(quote!(sequelite::sql_types::SqliteFlag::NotNull));
            }
        }

        // Get type of field ensuring that if it is an Option<T>, we get the inner type
        let field_type = if field_option {
            match field_type {
                syn::Type::Path(syn::TypePath { path, .. }) => {
                    let segments = &path.segments;
                    if segments.len() == 1 {
                        let segment = &segments[0];
                        let inner_type = &segment.arguments;
                        match inner_type {
                            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) => {
                                if args.len() == 1 {
                                    let arg = &args[0];
                                    match arg {
                                        syn::GenericArgument::Type(ty) => ty,
                                        _ => panic!("Only types are supported"),
                                    }
                                } else {
                                    panic!("Only one type is supported");
                                }
                            }
                            _ => panic!("Only types are supported"),
                        }
                    } else {
                        panic!("Only one type is supported");
                    }
                }
                _ => panic!("Only types are supported"),
            }
        } else {
            field_type
        };

        // Generate getter for column value
        let getter = if field_option {
            quote!(
                if column.name_const() == Self::#field_name.name_const() {
                    return self.#field_name.as_ref().map(|v| Box::new(v.clone()) as Box<dyn sequelite::model::SqliteToSql>)
                }
            )
        } else {
            quote!(
                if column.name_const() == Self::#field_name.name_const() {
                    return Some(Box::new(self.#field_name.clone()) as Box<dyn sequelite::model::SqliteToSql>)
                }
            )
        };
        column_value_getters.push(getter);

        // Generate setter for column value
        let setter = if field_option {
            quote!(
                #field_name: row.get(#i + offset).ok(),
            )
        } else {
            quote!(
                #field_name: row.get(#i + offset).unwrap(),
            )
        };
        column_value_setters.push(setter);

        let mut relation = quote!(None);

        // Get sqlitetype from field type
        let field_type = match field_type {
            syn::Type::Path(syn::TypePath { path, .. }) => {
                let segments = &path.segments;
                if segments.len() == 1 {
                    let segment = &segments[0];
                    let ident = &segment.ident;
                    // Check Vec<u8>
                    if ident.to_string() == "Vec" {
                        // Get inner type
                        let inner_type = &segment.arguments;

                        match inner_type {
                            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) => {
                                if args.len() == 1 {
                                    let arg = &args[0];
                                    match arg {
                                        syn::GenericArgument::Type(ty) => {
                                            let ty = match ty {
                                                syn::Type::Path(syn::TypePath { path, .. }) => {
                                                    let segments = &path.segments;
                                                    if segments.len() == 1 {
                                                        let segment = &segments[0];
                                                        let ident = &segment.ident;
                                                        ident.to_string()
                                                    } else {
                                                        panic!("Only one type is supported");
                                                    }
                                                }
                                                _ => panic!("Only types are supported"),
                                            };
                                            if ty == "u8" {
                                                quote!(sequelite::sql_types::SqliteType::Blob)
                                            } else {
                                                panic!("Unsupported type: {:?}", segments);
                                            }
                                        }
                                        _ => panic!("Only types are supported"),
                                    }
                                } else {
                                    panic!("Only one type is supported");
                                }
                            }
                            _ => panic!("Only types are supported"),
                        }
                    } else if ident.to_string() == "Relation" {
                        // Get inner type and save identifier
                        let inner_type = &segment.arguments;

                        let relation_type = match inner_type {
                            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) => {
                                if args.len() == 1 {
                                    let arg = &args[0];
                                    match arg {
                                        syn::GenericArgument::Type(ty) => {
                                            let ty = match ty {
                                                syn::Type::Path(syn::TypePath { path, .. }) => {
                                                    let segments = &path.segments;
                                                    if segments.len() == 1 {
                                                        let segment = &segments[0];
                                                        let ident = &segment.ident;
                                                        ident
                                                    } else {
                                                        panic!("Only one type is supported");
                                                    }
                                                }
                                                _ => panic!("Only types are supported"),
                                            };
                                            ty
                                        }
                                        _ => panic!("Only types are supported"),
                                    }
                                } else {
                                    panic!("Only one type is supported");
                                }
                            }
                            _ => panic!("Only types are supported"),
                        };

                        // Set relation
                        relation = quote!(Some(sequelite::model::relation::ColumnRelation::new(#relation_type::TABLE_NAME_CONST, Self::TABLE_NAME_CONST, "id", &#relation_type::ID_COLUMN_CONST, stringify!(#field_name))));

                        // And setter
                        column_value_setters[i] = quote!(
                            #field_name: Relation::<#relation_type>::parse_from_row(&row, offset, #i, &mut offset_counter, joins.contains(&stringify!(#field_name).to_string())),
                        );


                        // Get relation type
                        quote!(sequelite::sql_types::SqliteType::Integer)                        
                    } else {
                        // Other types
                        match ident.to_string().as_str() {
                            "String" => quote!(sequelite::sql_types::SqliteType::Text),
                            "i32" => quote!(sequelite::sql_types::SqliteType::Integer),
                            "i64" => quote!(sequelite::sql_types::SqliteType::Integer),
                            "f32" => quote!(sequelite::sql_types::SqliteType::Real),
                            "f64" => quote!(sequelite::sql_types::SqliteType::Real),
                            "bool" => quote!(sequelite::sql_types::SqliteType::Integer),
                            _ => panic!("Unsupported type: {:?}", segments),
                        }
                    }
                } else {
                    if segments.len() == 2 {
                        // Expect that 3rd segment is NaiveDateTime
                        let segment = &segments[1];
                        let ident = &segment.ident;
                        if ident.to_string() == "NaiveDateTime" {
                            quote!(sequelite::sql_types::SqliteType::DateTime)
                        } else {
                            panic!("Type {} not supported", ident.to_string());
                        }
                    } else {
                        panic!("Type {:?} not supported", segments);
                    }
                }
            }
            _ => panic!("Only types are supported"),
        };

        // If field has #[default(...)] attribute, set default value
        let mut default_value = quote!(None);
        for attr in &field.attrs {
            if attr.path.get_ident().unwrap().to_string() == "default_value" {
                let group = attr.tokens.clone().into_iter().next().unwrap();
                let group = match group {
                    proc_macro2::TokenTree::Group(group) => group,
                    _ => panic!("Invalid default_value attribute"),
                };

                let group = group.stream();
                default_value = quote!(Some(#group));
            }
        }

        quote!(
            pub const #field_name: sequelite::model::Column<'static> = 
                sequelite::model::Column::new_const(stringify!(#field_name), Self::TABLE_NAME_CONST, #field_type, &[#(#flags),*], #default_value, #relation);
        )
    });

    // Get table name from #[table_name = "table_name"] attribute on struct or use struct name if not present
    let table_name = match get_table_name(&ast.attrs) {
        Some(name) => name,
        None => name.to_string().to_lowercase() + "s",
    };

    quote!(
        #[allow(non_upper_case_globals)]
        impl #name {
            #(#field_consts)*

            pub const COLUMNS_SLICE: &'static [sequelite::model::Column<'static>] = &[
                #(#name::#field_names),*
            ];

            pub const TABLE_NAME_CONST: &'static str = #table_name;
            pub const ID_COLUMN_CONST: &'static sequelite::model::Column<'static> = &#id_column_const;
            pub const FIELDS_NUM_CONST: usize = #fields_num;
        }

        impl sequelite::model::Model for #name {
            fn table_name() -> &'static str {
                #table_name
            }

            fn columns() -> &'static [sequelite::model::Column<'static>] {
                #name::COLUMNS_SLICE
            }

            fn count_columns() -> usize {
                Self::FIELDS_NUM_CONST
            }

            fn get_id(&self) -> i64 {
                #id_column.unwrap() as i64
            }

            fn id_column() -> sequelite::model::Column<'static> {
                #id_column_const
            }

            fn column_value(&self, column: &'static sequelite::model::Column<'static>) -> Option<Box<dyn sequelite::model::SqliteToSql>> {
                // Todo: Make this more efficient than if ladder
                #(#column_value_getters)*

                // This should never happen
                panic!("Unknown column: {}", column.name_const());
            }

            fn parse_row(row: &sequelite::model::SqliteRow, offset: usize, joins: &Vec<String>) -> Self {
                let mut offset_counter = Self::FIELDS_NUM_CONST;
                Self {
                    #(#column_value_setters)*
                }
            }

            fn parse_rows(mut rows: sequelite::model::SqliteRows, offset: usize, joins: &Vec<String>) -> Vec<Self> {
                let mut temp = Vec::new();
                let mut row_counter = 0;

                while let Some(row) = rows.next().unwrap() {
                    temp.push(Self::parse_row(&row, offset, joins));

                    row_counter += 1;
                }
                temp
            }
        }
    ).into()
}

fn get_table_name(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path.get_ident().unwrap().to_string() == "table_name" {
            // Expect = symbol and string literal
            let mut tokens = attr.tokens.clone().into_iter();

            match tokens.next().unwrap() {
                proc_macro2::TokenTree::Punct(punct) => {
                    if punct.as_char() != '=' {
                        panic!("Expected '=' symbol");
                    }
                }
                _ => panic!("Expected '=' symbol"),
            }

            match tokens.next().unwrap() {
                proc_macro2::TokenTree::Literal(literal) => {
                    let literal = literal.to_string();
                    let literal = literal.trim_matches('"');
                    return Some(literal.to_string());
                }
                _ => panic!("Expected string literal"),
            }
        }
    }

    None
}

fn is_option(field_type: &syn::Type) -> bool {
    match field_type {
        syn::Type::Path(syn::TypePath { path, .. }) => {
            let segments = &path.segments;
            if segments.len() == 1 {
                let segment = &segments[0];
                segment.ident == "Option"
            } else {
                false
            }
        }
        _ => false,
    }
}