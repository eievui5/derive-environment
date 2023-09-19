#![doc = include_str!("../README.md")]

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
}

/// Generates a `load_environment()` function that will populate each field from environment variables.
///
/// This uses `.parse()` so make sure all members implement `FromStr`.
/// Note that `load_environment()` is not a constructor.
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
                ::std::result::Result::Ok(found_match)
            }
        }
    };

    // Hand the output tokens back to the compiler
    expanded.into()
}

fn to_variable(field: &&EnvFieldArgs) -> String {
    field
        .ident
        .clone()
        .unwrap()
        .to_string()
        .to_case(Case::UpperSnake)
}

fn to_field(field: &&EnvFieldArgs) -> Ident {
    field.ident.clone().unwrap()
}

fn get_type(f: &EnvFieldArgs) -> String {
    if let Type::Path(TypePath { qself: _, path }) = &f.ty {
        path.segments.first().unwrap().ident.to_string()
    } else {
        String::new()
    }
}

fn env_from_parseable(fields: &[&EnvFieldArgs]) -> TokenStream {
    let mut tokens = TokenStream::new();

    for field in fields {
        let ty = get_type(field);
        let f = to_field(field);
        let var = to_variable(field);

        // This is made a lot more complicated by the fact that we very little information about the types we recieve.
        // At best, we can guess using the first segment of the type path.
        match (field, ty.as_str()) {
            // Ignored fields
            (EnvFieldArgs { ignore: true , .. }, _) => {}
            // Vector & Nested
            (EnvFieldArgs { nested: true, .. }, "Vec") => tokens.extend(quote! {
                for i in 0.. {
                    let underscore_var = &::std::format!("{prefix}{}__{i}__", #var);

                    self.#f.extend([Default::default()]);

                    if self.#f.last_mut().unwrap().load_environment_with_prefix(&underscore_var)? {
                        found_match = true;
                    } else {
                        self.#f.pop();
                        break;
                    }
                }
            }),
            (EnvFieldArgs { .. }, "Vec") => tokens.extend(quote! {
                for i in 0.. {
                    let underscore_var = &::std::format!("{prefix}{}__{i}", #var);

                    if let ::std::result::Result::Ok(variable) = ::std::env::var(underscore_var) {
                        match variable.parse() {
                            ::std::result::Result::Ok(value) => {
                                self.#f.push(value);
                                found_match = true;
                            }
                            ::std::result::Result::Err(msg) => return ::std::result::Result::Err(::std::format!("{}: {}", underscore_var, msg.to_string())),
                        }
                    } else {
                        break;
                    }
                }
            }),
            // Optional & Nested
            (EnvFieldArgs { nested: true, .. }, "Option") => tokens.extend(quote! {
                let colon_var = ::std::format!("{prefix}{}:", #var);
                let underscore_var = ::std::format!("{prefix}{}__", #var);
                if let Some(field) = &mut self.#f {
                    if field.load_environment_with_prefix(&colon_var)? || self.#f.as_mut().unwrap().load_environment_with_prefix(&underscore_var)? {
                        found_match = true;
                    }
                } else {
                    self.#f = Some(Default::default());
                    if self.#f.as_mut().unwrap().load_environment_with_prefix(&colon_var)? || self.#f.as_mut().unwrap().load_environment_with_prefix(&underscore_var)? {
                        found_match = true;
                    } else {
                        self.#f = None;
                    }
                }
            }),
            // Optional & Parseable
            (EnvFieldArgs { nested: false, .. }, "Option") => tokens.extend(quote! {
                let name = ::std::format!("{prefix}{}", #var);

                if let ::std::result::Result::Ok(variable) = ::std::env::var(&name) {
                    match variable.parse() {
                        ::std::result::Result::Ok(value) => {
                            found_match = true;
                            self.#f = Some(value);
                        }
                        ::std::result::Result::Err(msg) => return ::std::result::Result::Err(::std::format!("{name}: {msg}")),
                    }
                }
            }),
            // Nested
            (EnvFieldArgs { nested: true, .. }, _) => tokens.extend(quote! {
                let underscore_var = ::std::format!("{prefix}{}__", #var);

                if self.#f.load_environment_with_prefix(&underscore_var)? {
                    found_match = true;
                }
            }),
            // Parseable
            (EnvFieldArgs { .. }, _) => tokens.extend(quote! {
                let name = ::std::format!("{prefix}{}", #var);

                if let ::std::result::Result::Ok(variable) = ::std::env::var(&name) {
                    match variable.parse() {
                        ::std::result::Result::Ok(value) => {
                            found_match = true;
                            self.#f = value;
                        }
                        ::std::result::Result::Err(msg) => return ::std::result::Result::Err(::std::format!("{name}: {msg}")),
                    }
                }
            }),
        }
    }

    tokens
}
