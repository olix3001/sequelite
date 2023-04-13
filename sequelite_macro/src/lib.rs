use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(Model, attributes(default, table_name))]
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
                    return self.#field_name.map(|v| Box::new(v.clone()) as Box<dyn sequelite::model::SqliteToSql>)
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
                #field_name: row.get(#i).ok(),
            )
        } else {
            quote!(
                #field_name: row.get(#i).unwrap(),
            )
        };
        column_value_setters.push(setter);

        // Get sqlitetype from field type
        let field_type = match field_type {
            syn::Type::Path(syn::TypePath { path, .. }) => {
                let segments = &path.segments;
                if segments.len() == 1 {
                    let segment = &segments[0];
                    let ident = &segment.ident;
                    match ident.to_string().as_str() {
                        "String" => quote!(sequelite::sql_types::SqliteType::Text),
                        "i32" => quote!(sequelite::sql_types::SqliteType::Integer),
                        "i64" => quote!(sequelite::sql_types::SqliteType::Integer),
                        "f32" => quote!(sequelite::sql_types::SqliteType::Real),
                        "f64" => quote!(sequelite::sql_types::SqliteType::Real),
                        "bool" => quote!(sequelite::sql_types::SqliteType::Integer),
                        _ => panic!("Unsupported type"),
                    }
                } else {
                    panic!("Only one type is supported");
                }
            }
            _ => panic!("Only types are supported"),
        };

        // If field has #[default(...)] attribute, set default value
        let mut default_value = quote!(None);
        for attr in &field.attrs {
            if attr.path.get_ident().unwrap().to_string() == "default" {
                let group = attr.tokens.clone().into_iter().next().unwrap();
                let group = match group {
                    proc_macro2::TokenTree::Group(group) => group,
                    _ => panic!("Invalid default value"),
                };

                let group = group.stream();
                default_value = quote!(Some(#group));
            }
        }


        quote!(
            pub const #field_name: sequelite::model::Column<'static> = 
                sequelite::model::Column::new_const(stringify!(#field_name), #field_type, &[#(#flags),*], #default_value);
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
        }

        impl sequelite::model::Model for #name {
            fn table_name() -> &'static str {
                #table_name
            }

            fn columns() -> &'static [sequelite::model::Column<'static>] {
                #name::COLUMNS_SLICE
            }

            fn column_value(&self, column: &'static sequelite::model::Column<'static>) -> Option<Box<dyn sequelite::model::SqliteToSql>> {
                // Todo: Make this more efficient than if ladder
                #(#column_value_getters)*

                // This should never happen
                panic!("Unknown column: {}", column.name_const());
            }

            fn parse_rows(mut rows: sequelite::model::SqliteRows) -> Vec<Self> {
                let mut temp = Vec::new();
                while let Some(row) = rows.next().unwrap() {
                    temp.push(Self {
                        #(#column_value_setters)*
                    });
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