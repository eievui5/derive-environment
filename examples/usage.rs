#![allow(dead_code)]

use derive_environment::FromEnv;

#[derive(Clone, Debug, Default)]
struct UnparsableStruct;

#[derive(Clone, Debug, Default, FromEnv)]
struct SubStruct {
	value: u16,
}

#[derive(Clone, Debug, Default, FromEnv)]
struct Struct {
	parseable: String,
	#[env(ignore)]
	ignored: UnparsableStruct,
	nested: SubStruct,
	vector: Vec<String>,
	nested_vector: Vec<SubStruct>,
	optional: Option<String>,
}

fn main() {
	let mut test = Struct::default();
	test.with_env("TEST_PREFIX").unwrap();
	println!("{test:#?}");
}
