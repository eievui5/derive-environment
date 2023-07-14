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
/// If a certain field should not be configurable via environment variables, mark it with `#[env(ignore)]`.
///
/// # Nested fields
///
/// By default, fields are parsed using the FromStr trait.
/// This can be a problem when you have a nested struct and only want to change one of its fields.
/// To mark a field as nested, first `#[derive(Environment)]` on the sub-structure.
/// Then mark the field as `#[env(nested)]`.
/// 
/// # Extendable fields
///
/// If a field implements the `Extend` trait, like `Vec` or `VecDeque`,
/// you can use the `#[env(extendable)]` annotation to configure the field by index.
/// 
/// If the collection contains a nested field, you can use `#[env(nested_extendable)]` instead.
/// Note that types are constructed in-place, and some fields may be missing from the environment.
/// Because of this, the contents of the collection must implement the `Default` trait.
/// You can derive it with `#[derive(Default)]`.
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
/// <hr>
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
/// <hr>
///
/// Vector of Nested fields:
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
///     #[env(nested_extendable)]
///     server: Vec<ServerConfig>,
/// }
/// ```
///
/// Generates:
/// - MY_CONFIG_SERVER:0:PORT
/// - MY_CONFIG_SERVER__0__PORT
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

    macro_rules! make_filter {
        ($name:ident, $on_match:ident, $matches:expr) => {
            fn $name(field: &&Field) -> bool {
                for attr in &field.attrs {
                    if let Meta::List(ref attr_token) = attr.meta {
                        if *attr_token.path.get_ident().unwrap() == "env" {
                            for token in attr_token.tokens.clone() {
                                if $matches(token.to_string().as_str()) {
                                    return $on_match;
                                }
                            }
                        }
                    }
                }
                !$on_match
            }
        };
    }

    fn to_variables(field: &Field) -> String {
        field
            .ident
            .clone()
            .unwrap()
            .to_string()
            .to_case(Case::UpperSnake)
    }

    make_filter!(remove_ignored, false, |token| matches!(token, "ignore" | "nested" | "extendable" | "nested_extendable"));
    make_filter!(only_nested, true, |token| token == "nested");
    make_filter!(only_extendable, true, |token| token == "extendable");
    make_filter!(only_nested_extendable, true, |token| token == "nested_extendable");

    let extendable_fields: Vec<Ident> = fields
        .named
        .iter()
        .filter(only_extendable)
        .map(|field| field.ident.clone().unwrap())
        .collect();

    let extendable_variables: Vec<String> = fields
        .named
        .iter()
        .filter(only_extendable)
        .map(to_variables)
        .collect();

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
        .map(to_variables)
        .collect();

    let nested_extendable_fields: Vec<Ident> = fields
        .named
        .iter()
        .filter(only_nested_extendable)
        .map(|field| field.ident.clone().unwrap())
        .collect();

    let nested_extendable_variables: Vec<String> = fields
        .named
        .iter()
        .filter(only_nested_extendable)
        .map(to_variables)
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
        .map(to_variables)
        .collect();

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        impl #name {
            fn load_environment(&mut self) -> Result<bool, String> {
                self.load_environment_with_prefix(#prefix)
            }

            fn load_environment_with_prefix(&mut self, prefix: &str) -> Result<bool, String> {
                // Tracks whether or not a variable was found.
                // Important for nested extendables.
                let mut found_match = false;
                // default
                #({
                    let name = format!("{prefix}{}", #variable_names);
                    #[cfg(feature = "debug")]
                    println!("{name}");
                    if let Ok(variable) = ::std::env::var(&name) {
                        match variable.parse() {
                            Ok(value) => {
                                found_match = true;
                                self.#field_names = value;
                            }
                            Err(msg) => return Err(format!("{}: {}", name, msg.to_string())),
                        }
                    }
                })*
                // nested
                #({
                    let colon_var = format!("{prefix}{}:", #nested_variables);
                    let underscore_var = format!("{prefix}{}__", #nested_variables);
                    if self.#nested_fields.load_environment_with_prefix(&colon_var)? {
                        found_match = true;
                    }
                    if self.#nested_fields.load_environment_with_prefix(&underscore_var)? {
                        found_match = true;
                    }
                })*
                // extendable
                #({
                    for i in 0.. {
                        let colon_var = &format!("{prefix}{}:{i}", #extendable_variables);
                        let underscore_var = &format!("{prefix}{}__{i}", #extendable_variables);
                        #[cfg(feature = "debug")]
                        println!("{colon_var}");

                        if let Ok(variable) = ::std::env::var(colon_var) {
                            match variable.parse() {
                                Ok(value) => {
                                    self.#extendable_fields.extend([value].iter());
                                    found_match = true;
                                }
                                Err(msg) => return Err(format!("{}: {}", colon_var, msg.to_string())),
                            }
                        } else if let Ok(variable) = ::std::env::var(underscore_var) {
                            match variable.parse() {
                                Ok(value) => {
                                    self.#extendable_fields.extend([value].iter());
                                    found_match = true;
                                }
                                Err(msg) => return Err(format!("{}: {}", underscore_var, msg.to_string())),
                            }
                        } else {
                            break;
                        }
                    }
                })*
                // nested & extendable
                #({
                    for i in 0.. {
                        let colon_var = &format!("{prefix}{}:{i}:", #nested_extendable_variables);
                        let underscore_var = &format!("{prefix}{}__{i}__", #nested_extendable_variables);

                        self.#nested_extendable_fields.extend([Default::default()]);

                        if self.#nested_extendable_fields.last_mut().unwrap().load_environment_with_prefix(&colon_var)? {
                            found_match = true;
                        } else if self.#nested_extendable_fields.last_mut().unwrap().load_environment_with_prefix(&underscore_var)? {
                            found_match = true;
                        } else {
                            self.#nested_extendable_fields.pop();
                            break;
                        }
                    }
                })*
                Ok(found_match)
            }
        }
    };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}
