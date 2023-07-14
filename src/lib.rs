use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::quote;
use syn::*;

/// Generates a `load_environment()` function that will populate each field from environment variables.
///
/// This uses `.parse()` so make sure all members implement `FromStr`.
/// Note that `load_environment()` is not a constructor.
///
/// # Ignored fields
///
/// If a certain field should not be configurable via environment variables, mark it with `#[env(ignore)]`.Build
///
/// # Nested fields
///
/// By default, fields are parsed using the FromStr trait.
/// This can be a problem when you have a nested struct and only want to change one of its fields.
/// To mark a field as nested, first `#[derive(Environment)]` on the sub-structure.
/// Then mark the field as `#[env(nested)]`.
/// 
///
/// # Examples
/// 
/// Creating a config file:
///
/// ```
/// use derive_environment::Environment;
/// 
/// #[derive(Environment)]
/// #[prefix(HL7_)] // or whatever you want
/// pub struct Config {
///     // ...
/// }
/// ```
///
/// Nesting fields:
///
/// ```
/// use derive_environment::Environment;
/// 
/// #[derive(Environment)]
/// struct ServerConfig {
///     port: u16,
/// }
///
/// #[derive(Environment)]
/// #[prefix(MY_CONFIG_)]
/// pub struct Config {
///     #[env(nested)]
///     server: ServerConfig,
/// }
/// ```
///
/// Generates:
/// - MY_CONFIG_SERVER:PORT
/// - MY_CONFIG_SERVER__PORT
///
#[proc_macro_derive(Environment, attributes(prefix, env))]
pub fn environment(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let mut prefix = String::new();
    for i in input.attrs {
        if let Meta::List(ref attr_token) = i.meta {
            if *attr_token.path.get_ident().unwrap() == "prefix" {
                prefix = attr_token.tokens.to_string();
                break;
            }
        }
    }

    let Data::Struct(data) = input.data else {
        panic!("Environment can only be derived for structures");
    };

    let Fields::Named(fields) = data.fields else {
        panic!("Structure fields must be named to derive Environment");
    };

    fn remove_ignored(field: &&Field) -> bool {
        for attr in &field.attrs {
            if let Meta::List(ref attr_token) = attr.meta {
                if *attr_token.path.get_ident().unwrap() == "env" {
                    for token in attr_token.tokens.clone() {
                        if matches!(
                            token.to_string().as_ref(),
                            "ignore" | "nested"
                        ) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    fn only_nested(field: &&Field) -> bool {
        for attr in &field.attrs {
            if let Meta::List(ref attr_token) = attr.meta {
                if *attr_token.path.get_ident().unwrap() == "env" {
                    for token in attr_token.tokens.clone() {
                        if matches!(
                            token.to_string().as_ref(),
                            "nested"
                        ) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    let nested_fields: Vec<Ident> = fields
        .named
        .iter()
        .filter(only_nested)
        .map(|field| field.ident.clone().unwrap())
        .collect();

    let nested_variables: Vec<String> = fields
        .named
        .iter()
        .filter(only_nested)
        .map(|field| {
            field
                .ident
                .clone()
                .unwrap()
                .to_string()
                .to_case(Case::UpperSnake)
        })
        .collect();

    let field_names: Vec<Ident> = fields
        .named
        .iter()
        .filter(remove_ignored)
        .map(|field| field.ident.clone().unwrap())
        .collect();

    let variable_names: Vec<String> = fields
        .named
        .iter()
        .filter(remove_ignored)
        .map(|field| {
            field
                .ident
                .clone()
                .unwrap()
                .to_string()
                .to_case(Case::UpperSnake)
        })
        .collect();

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        impl #name {
            fn load_environment(&mut self) -> Result<(), String> {
                self.load_environment_with_prefix(#prefix)
            }

            fn load_environment_with_prefix(&mut self, prefix: &str) -> Result<(), String> {
                #({
                    let name = format!("{prefix}{}", #variable_names);
                    #[cfg(feature = "debug")]
                    println!("{name}");
                    if let Ok(variable) = ::std::env::var(&name) {
                        match variable.parse() {
                            Ok(value) => self.#field_names = value,
                            Err(msg) => return Err(format!("{}: {}", name, msg.to_string())),
                        }
                    }
                })*
                #({
                    self.#nested_fields.load_environment_with_prefix(&format!("{prefix}{}:", #nested_variables))?;
                    self.#nested_fields.load_environment_with_prefix(&format!("{prefix}{}__", #nested_variables))?;
                })*
                Ok(())
            }
        }
    };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}
