use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::quote;
use syn::*;

#[proc_macro_derive(Environment, attributes(prefix))]
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

    let field_names: Vec<Ident> = fields
        .named
        .iter()
        .map(|field| field.ident.clone().unwrap())
        .collect();
    let variable_names: Vec<String> = fields
        .named
        .iter()
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
                #({
                    let name = concat!(#prefix, #variable_names);
                    if let Ok(variable) = ::std::env::var(name) {
                        match variable.parse() {
                            Ok(value) => self.#field_names = value,
                            Err(msg) => return Err(format!("{}: {}", name.to_string(), msg.to_string())),
                        }
                    }
                })*
                Ok(())
            }
        }
    };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}
