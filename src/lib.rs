use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::quote;
use syn::*;

macro_rules! if_env {
    ($attrs:expr => $do:expr) => {
        for attr in $attrs {
            if let Meta::List(ref attr_token) = attr.meta {
                if *attr_token.path.get_ident().unwrap() == "env" {
                    $do(attr_token);
                }
            }
        }
    }
}

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
/// #[env(prefix = HL7_)] // or whatever you want
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
/// #[env(prefix = MY_CONFIG_)]
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
/// #[env(prefix = MY_CONFIG_)]
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
pub fn environment(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let mut prefix = String::new();

    // This syntax should be considered deprecated.
    for i in &input.attrs {
        if let Meta::List(ref attr_token) = i.meta {
            if *attr_token.path.get_ident().unwrap() == "prefix" {
                prefix = attr_token.tokens.to_string();
                break;
            }
        }
    }

    if_env!(&input.attrs => |attr: &MetaList| {
        let tks = &mut attr.tokens.clone().into_iter();
        while let Some(tk) = tks.next() {
            match tk.to_string().as_ref() {
                "prefix" => {
                    let eq = tks.next().expect("expected `=` following `prefix`").to_string();
                    if eq != "=" {
                        panic!("expected = following `prefix`");
                    }
                    prefix = tks.next().expect("expected prefix token").to_string();
                }
                unexpected => {
                    panic!("Unexpected parameter: {unexpected}");
                }
            }
        }
    });

    let Data::Struct(data) = input.data else {
        panic!("Environment can only be derived for structures");
    };

    let Fields::Named(fields) = data.fields else {
        panic!("Structure fields must be named to derive Environment");
    };

    let parseable_fields = env_from_parseable(&fields);
    let nested_fields = env_from_nested(&fields);
    let extendable_fields = env_from_extendable(&fields);
    let nested_extendable = env_from_nested_extendable(&fields);

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        impl #name {
            /// Modifies the config using environment variables with the default prefix.
            /// Returns whether or not the structure was modified.
            /// # Errors
            /// Returns an error if any variables could not be read.
            fn load_environment(&mut self) -> Result<bool, String> {
                self.load_environment_with_prefix(#prefix)
            }

            /// Modifies the config using environment variables with a given prefix.
            /// Returns whether or not the structure was modified.
            /// # Errors
            /// Returns an error if any variables could not be read.
            fn load_environment_with_prefix(&mut self, prefix: &str) -> Result<bool, String> {
                // Tracks whether or not a variable was found.
                // Important for nested extendables.
                let mut found_match = false;

                #parseable_fields
                #nested_fields
                #extendable_fields
                #nested_extendable

                Ok(found_match)
            }
        }
    };

    // Hand the output tokens back to the compiler
    expanded.into()
}

fn to_variables(field: &Field) -> String {
    field
        .ident
        .clone()
        .unwrap()
        .to_string()
        .to_case(Case::UpperSnake)
}

fn to_fields(field: &Field) -> Ident {
    field.ident.clone().unwrap()
}

make_filter!(remove_ignored, false, |token| matches!(
    token,
    "ignore" | "nested" | "extendable" | "nested_extendable"
));
make_filter!(only_nested, true, |token| token == "nested");
make_filter!(only_extendable, true, |token| token == "extendable");
make_filter!(only_nested_extendable, true, |token| token
    == "nested_extendable");

fn convert_fields<'a>(
    fields: &'a FieldsNamed,
    filter: &'a (impl Fn(&&Field) -> bool + 'a),
) -> (
    impl Iterator<Item = Ident> + 'a,
    impl Iterator<Item = String> + 'a,
) {
    (
        fields.named.iter().filter(filter).map(to_fields),
        fields.named.iter().filter(filter).map(to_variables),
    )
}

fn env_from_parseable(fields: &FieldsNamed) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &remove_ignored);

    quote! {#({
        let name = format!("{prefix}{}", #vars);
        #[cfg(feature = "debug")]
        println!("{name}");
        if let Ok(variable) = ::std::env::var(&name) {
            match variable.parse() {
                Ok(value) => {
                    found_match = true;
                    self.#fields = value;
                }
                Err(msg) => return Err(format!("{}: {}", name, msg.to_string())),
            }
        }
    })*}
}

fn env_from_nested(fields: &FieldsNamed) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &only_nested);

    quote! {#({
        let colon_var = format!("{prefix}{}:", #vars);
        let underscore_var = format!("{prefix}{}__", #vars);
        if self.#fields.load_environment_with_prefix(&colon_var)? {
            found_match = true;
        }
        if self.#fields.load_environment_with_prefix(&underscore_var)? {
            found_match = true;
        }
    })*}
}

fn env_from_extendable(fields: &FieldsNamed) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &only_extendable);

    quote! {#({
        for i in 0.. {
            let colon_var = &format!("{prefix}{}:{i}", #vars);
            let underscore_var = &format!("{prefix}{}__{i}", #vars);
            #[cfg(feature = "debug")]
            println!("{colon_var}");

            if let Ok(variable) = ::std::env::var(colon_var) {
                match variable.parse() {
                    Ok(value) => {
                        self.#fields.extend([value].iter());
                        found_match = true;
                    }
                    Err(msg) => return Err(format!("{}: {}", colon_var, msg.to_string())),
                }
            } else if let Ok(variable) = ::std::env::var(underscore_var) {
                match variable.parse() {
                    Ok(value) => {
                        self.#fields.extend([value].iter());
                        found_match = true;
                    }
                    Err(msg) => return Err(format!("{}: {}", underscore_var, msg.to_string())),
                }
            } else {
                break;
            }
        }
    })*}
}

fn env_from_nested_extendable(fields: &FieldsNamed) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &only_nested_extendable);

    quote! {#({
        for i in 0.. {
            let colon_var = &format!("{prefix}{}:{i}:", #vars);
            let underscore_var = &format!("{prefix}{}__{i}__", #vars);

            self.#fields.extend([Default::default()]);

            if self.#fields.last_mut().unwrap().load_environment_with_prefix(&colon_var)? {
                found_match = true;
            } else if self.#fields.last_mut().unwrap().load_environment_with_prefix(&underscore_var)? {
                found_match = true;
            } else {
                self.#fields.pop();
                break;
            }
        }
    })*}
}
