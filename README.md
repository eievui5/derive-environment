A Rust library for modifying structs via environment variables.

Unlike [envy](https://github.com/softprops/envy), this function does *not* create a new object.
It is used to reconfigure an existing structure (after having parsed it from a config file, for example).

# Ignored fields

If a certain field should not be configurable via environment variables, mark it with `#[env(ignore)]`.

# Nested fields

By default, fields are parsed using the FromStr trait.
This can be a problem when you have a nested struct and only want to change one of its fields.
To mark a field as nested, first `#[derive(Environment)]` on the sub-structure.
Then mark the field as `#[env(nested)]`.

# Extendable fields

If a field implements the `Extend` trait, like `Vec` or `VecDeque`,
you can use the `#[env(extendable)]` annotation to configure the field by index.
If the collection contains a nested field, you can use `#[env(nested, extendable)]` together.
Note that types are constructed in-place, and some fields may be missing from the environment.
Because of this, the contents of the collection must implement the `Default` trait.
You can derive it with `#[derive(Default)]`.

# Examples

Creating a config file:

```
use derive_environment::Environment;
#[derive(Environment)]
#[env(prefix = "HL7_")] // or whatever you want
pub struct Config {
    // ...
}
```

<hr>

Nesting fields:

```
use derive_environment::Environment;
#[derive(Environment)]
struct ServerConfig {
    port: u16,
}
#[derive(Environment)]
#[env(prefix = "MY_CONFIG_")]
pub struct Config {
    #[env(nested)]
    server: ServerConfig,
}
```

Generates:
- MY_CONFIG_SERVER:PORT
- MY_CONFIG_SERVER__PORT

<hr>

Vector of Nested fields:

```
use derive_environment::Environment;
#[derive(Environment)]
struct ServerConfig {
    port: u16,
}
#[derive(Environment)]
#[env(prefix = "MY_CONFIG_")]
pub struct Config {
    #[env(nested, extendable)]
    server: Vec<ServerConfig>,
}
```

Generates:
- MY_CONFIG_SERVER:0:PORT
- MY_CONFIG_SERVER__0__PORT
