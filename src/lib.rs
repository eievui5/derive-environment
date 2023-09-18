use convert_case::{Case, Casing};
use darling::{ast, FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use quote::quote;
use syn::*;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(env), supports(struct_named))]
#[allow(dead_code)]
struct EnvArgs {
    ident: syn::Ident,
    generics: syn::Generics,
    data: ast::Data<(), EnvFieldArgs>,
    #[darling(default)]
    prefix: ::std::string::String,
    #[darling(default)]
    from_env: bool,
}

#[derive(Debug, FromField)]
#[darling(attributes(env))]
#[allow(dead_code)]
struct EnvFieldArgs {
    ident: Option<syn::Ident>,
    ty: syn::Type,

    #[darling(default)]
    ignore: bool,
    #[darling(default)]
    nested: bool,
    #[darling(default)]
    extendable: bool,
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
/// If the collection contains a nested field, you can use `#[env(nested, extendable)]` together.
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
/// #[env(prefix = "HL7_")] // or whatever you want
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
/// #[env(prefix = "MY_CONFIG_")]
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
/// #[env(prefix = "MY_CONFIG_")]
/// pub struct Config {
///     #[env(nested, extendable)]
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

    let args = match EnvArgs::from_derive_input(&input) {
        Ok(v) => v,
        Err(e) => {
            return e.write_errors().into();
        }
    };

    let name = input.ident;
    let prefix = args.prefix;

    let fields = args.data.as_ref().take_struct().unwrap().fields;

    let from_env = if args.from_env {
        quote! {
            /// Creates a new object with its fields initialized using the environment.
            pub fn from_env() -> ::std::result::Result<Self, ::std::string::String> {
                let mut s = Self::default();
                s.load_environment()?;
                Ok(s)
            }
        }
    } else {
        quote! {}
    };

    let parseable_fields = env_from_parseable(&fields);
    let optional_fields = env_from_optional(&fields);
    let nested_fields = env_from_nested(&fields);
    let optional_nested_fields = env_from_optional_nested(&fields);
    let extendable_fields = env_from_extendable(&fields);
    let nested_extendable = env_from_nested_extendable(&fields);

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        impl #name {
            #from_env

            /// Modifies the config using environment variables with the default prefix.
            /// Returns whether or not the structure was modified.
            /// # Errors
            /// Returns an error if any variables could not be read.
            pub fn load_environment(&mut self) -> ::std::result::Result<bool, ::std::string::String> {
                self.load_environment_with_prefix(#prefix)
            }

            /// Modifies the config using environment variables with a given prefix.
            /// Returns whether or not the structure was modified.
            /// # Errors
            /// Returns an error if any variables could not be read.
            pub fn load_environment_with_prefix(&mut self, prefix: &str) -> ::std::result::Result<bool, ::std::string::String> {
                // Tracks whether or not a variable was found.
                // Important for nested extendables.
                let mut found_match = false;

                #parseable_fields
                #optional_fields
                #nested_fields
                #optional_nested_fields
                #extendable_fields
                #nested_extendable

                ::std::result::Result::Ok(found_match)
            }
        }
    };

    // Hand the output tokens back to the compiler
    expanded.into()
}

fn to_variables(field: &&EnvFieldArgs) -> String {
    field
        .ident
        .clone()
        .unwrap()
        .to_string()
        .to_case(Case::UpperSnake)
}

fn to_fields(field: &&EnvFieldArgs) -> Ident {
    field.ident.clone().unwrap()
}

fn convert_fields<'a>(
    fields: &'a [&'a EnvFieldArgs],
    filter: &'a (impl Fn(&&&EnvFieldArgs) -> bool + 'a),
) -> (
    impl Iterator<Item = Ident> + 'a,
    impl Iterator<Item = String> + 'a,
) {
    (
        fields
            .iter()
            .filter(|f| f.ident.is_some())
            .filter(filter)
            .map(to_fields),
        fields
            .iter()
            .filter(|f| f.ident.is_some())
            .filter(filter)
            .map(to_variables),
    )
}

fn is_option(f: &EnvFieldArgs) -> bool {
    if let Type::Path(TypePath { qself: _, path }) = &f.ty {
        path.segments.first().unwrap().ident == "Option"
    } else {
        false
    }
}

fn env_from_parseable(fields: &[&EnvFieldArgs]) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &|f: &&&EnvFieldArgs| {
        !(f.ignore || f.nested || f.extendable || is_option(f))
    });

    quote! {#({
        let name = ::std::format!("{prefix}{}", #vars);
        if let ::std::result::Result::Ok(variable) = ::std::env::var(&name) {
            match variable.parse() {
                ::std::result::Result::Ok(value) => {
                    found_match = true;
                    self.#fields = value;
                }
                ::std::result::Result::Err(msg) => return ::std::result::Result::Err(::std::format!("{}: {}", name, msg.to_string())),
            }
        }
    })*}
}

fn env_from_optional(fields: &[&EnvFieldArgs]) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &|f: &&&EnvFieldArgs| {
        !(f.ignore || f.nested || f.extendable) && is_option(f)
    });

    quote! {#({
        let name = ::std::format!("{prefix}{}", #vars);
        if let ::std::result::Result::Ok(variable) = ::std::env::var(&name) {
            match variable.parse() {
                ::std::result::Result::Ok(value) => {
                    found_match = true;
                    self.#fields = Some(value);
                }
                ::std::result::Result::Err(msg) => return ::std::result::Result::Err(::std::format!("{}: {}", name, msg.to_string())),
            }
        }
    })*}
}

fn env_from_nested(fields: &[&EnvFieldArgs]) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &|f: &&&EnvFieldArgs| {
        f.nested && !f.extendable && !is_option(f)
    });

    quote! {#({
        let colon_var = ::std::format!("{prefix}{}:", #vars);
        let underscore_var = ::std::format!("{prefix}{}__", #vars);
        if self.#fields.load_environment_with_prefix(&colon_var)? {
            found_match = true;
        }
        if self.#fields.load_environment_with_prefix(&underscore_var)? {
            found_match = true;
        }
    })*}
}

fn env_from_optional_nested(fields: &[&EnvFieldArgs]) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &|f: &&&EnvFieldArgs| {
        f.nested && !f.extendable && is_option(f)
    });

    quote! {#({
        let colon_var = ::std::format!("{prefix}{}:", #vars);
        let underscore_var = ::std::format!("{prefix}{}__", #vars);
        if let Some(field) = &mut self.#fields {
            if field.load_environment_with_prefix(&colon_var)? || self.#fields.as_mut().unwrap().load_environment_with_prefix(&underscore_var)? {
                found_match = true;
            }
        } else {
            self.#fields = Some(Default::default());
            if self.#fields.as_mut().unwrap().load_environment_with_prefix(&colon_var)? || self.#fields.as_mut().unwrap().load_environment_with_prefix(&underscore_var)? {
                found_match = true;
            } else {
                self.#fields = None;
            }
        }
    })*}
}

fn env_from_extendable(fields: &[&EnvFieldArgs]) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &|f: &&&EnvFieldArgs| !f.nested && f.extendable);

    quote! {#({
        for i in 0.. {
            let colon_var = &::std::format!("{prefix}{}:{i}", #vars);
            let underscore_var = &::std::format!("{prefix}{}__{i}", #vars);

            if let ::std::result::Result::Ok(variable) = ::std::env::var(colon_var) {
                match variable.parse() {
                    ::std::result::Result::Ok(value) => {
                        self.#fields.push(value);
                        found_match = true;
                    }
                    ::std::result::Result::Err(msg) => return ::std::result::Result::Err(::std::format!("{}: {}", colon_var, msg.to_string())),
                }
            } else if let ::std::result::Result::Ok(variable) = ::std::env::var(underscore_var) {
                match variable.parse() {
                    ::std::result::Result::Ok(value) => {
                        self.#fields.push(value);
                        found_match = true;
                    }
                    ::std::result::Result::Err(msg) => return ::std::result::Result::Err(::std::format!("{}: {}", underscore_var, msg.to_string())),
                }
            } else {
                break;
            }
        }
    })*}
}

fn env_from_nested_extendable(fields: &[&EnvFieldArgs]) -> TokenStream {
    let (fields, vars) = convert_fields(fields, &|f: &&&EnvFieldArgs| f.nested && f.extendable);

    quote! {#({
        for i in 0.. {
            let colon_var = &::std::format!("{prefix}{}:{i}:", #vars);
            let underscore_var = &::std::format!("{prefix}{}__{i}__", #vars);

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
