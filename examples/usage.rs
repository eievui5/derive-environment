#![allow(dead_code)]

use derive_environment::Environment;

#[derive(Clone, Debug, Default)]
struct UnparsableStruct;

#[derive(Clone, Debug, Default, Environment)]
struct SubStruct {
    value: u16,
}

#[derive(Clone, Debug, Default, Environment)]
#[env(from_env, prefix = "TEST_PREFIX_")]
struct Struct {
    parseable: String,
    #[env(ignore)]
    ignored: UnparsableStruct,
    #[env(nested)]
    nested: SubStruct,
    vector: Vec<String>,
    #[env(nested)]
    nested_vector: Vec<SubStruct>,
    optional: Option<String>,
}

fn main() {
    let test = Struct::from_env().unwrap();
    println!("{test:#?}");
}
