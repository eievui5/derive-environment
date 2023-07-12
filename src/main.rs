use derive_environment::Environment;

#[derive(Clone, Debug, Environment)]
#[prefix(TEST_PREFIX_)]
struct Struct {
    pub name: String,
}

fn main() {
    let mut test = Struct {
        name: "yes".to_string(),
    };
    println!("{}", test.name);
    test.load_environment().unwrap();
    println!("{}", test.name);
}
