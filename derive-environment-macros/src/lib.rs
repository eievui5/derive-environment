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
}

#[derive(Debug, FromField)]
#[darling(attributes(env))]
#[allow(dead_code)]
struct EnvFieldArgs {
	ident: Option<syn::Ident>,
	ty: syn::Type,

	#[darling(default)]
	ignore: bool,
}

/// Generates a `load_environment()` function that will populate each field from environment variables.
///
/// This uses `.parse()` so make sure all members implement `FromStr`.
/// Note that `load_environment()` is not a constructor.
#[proc_macro_derive(FromEnv, attributes(prefix, env))]
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
	let fields = args.data.as_ref().take_struct().unwrap().fields;
	let parseable_fields = env_from_parseable(&fields);

	// Build the output, possibly using quasi-quotation
	let expanded = quote! {
		impl ::derive_environment::FromEnv for #name {
			fn with_env(&mut self, prefix: &str) -> ::derive_environment::Result<bool> {
				// Tracks whether or not a variable was found.
				// Important for nested extendables.
				let mut found_match = false;
				#parseable_fields
				::derive_environment::Result::Ok(found_match)
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

fn env_from_parseable(fields: &[&EnvFieldArgs]) -> TokenStream {
	let mut tokens = TokenStream::new();

	for field in fields.iter().filter(|x| !x.ignore) {
		let f = to_field(field);
		let var = to_variable(field);

		tokens.extend(quote! {
			let name = ::std::format!("{prefix}_{}", #var);

			if derive_environment::FromEnv::with_env(&mut self.#f, &name)? {
				found_match = true;
			}
		});
	}

	tokens
}
