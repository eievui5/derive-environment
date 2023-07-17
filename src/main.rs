#![allow(dead_code)]

use derive_environment::Environment;

#[derive(Clone, Debug, Default)]
struct UnparsableStruct;

#[derive(Clone, Debug, Default, Environment)]
struct SubStruct {
    port: u16,
}

#[derive(Clone, Debug, Default, Environment)]
#[env(prefix = TEST_PREFIX_)]
struct Struct {
    name: String,
    #[env(ignore)]
    ignored: UnparsableStruct,
    #[env(nested)]
    sub: SubStruct,
    #[env(extendable)]
    array: Vec<u32>,
    #[env(nested_extendable)]
    sub_structs: Vec<SubStruct>,
}

fn main() {
    let mut test = Struct::default();
    println!("{test:?}");
    test.load_environment().unwrap();
    println!("{test:?}");
}
