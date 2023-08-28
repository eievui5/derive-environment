#![allow(dead_code)]

use derive_environment::Environment;

#[derive(Clone, Debug, Default)]
struct UnparsableStruct;

#[derive(Clone, Debug, Default, Environment)]
struct SubStruct {
    port: u16,
}

#[derive(Clone, Debug, Default, Environment)]
#[env(from_env, prefix = "TEST_PREFIX_")]
struct Struct {
    name: String,
    #[env(ignore)]
    ignored: UnparsableStruct,
    #[env(nested)]
    sub: SubStruct,
    #[env(extendable)]
    array: Vec<u8>,
    #[env(extendable)]
    array_strings: Vec<String>,
    #[env(nested, extendable)]
    sub_structs: Vec<SubStruct>,
}

fn main() {
    let test = Struct::from_env().unwrap();
    println!("{test:#?}");
}
