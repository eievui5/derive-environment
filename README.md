A Rust library for modifying structs via environment variables.

Unlike [envy](https://github.com/softprops/envy), this does *not* create a new object.
It is used to reconfigure an existing structure (after having parsed it from a config file, for example).

# Ignored fields

If a certain field should not be configurable via environment variables, mark it with `#[env(ignore)]`.

# Examples

Creating a config structure:

```
use derive_environment::FromEnv;

#[derive(Default, FromEnv)]
pub struct Config {
    // ...
}

// Creates a base configuration struct to add on to.
// Normally this would be created using `serde` from a config file.
let mut config = Config::default();
// Names the struct "MY_CONFIG", which acts as a prefix.
config.with_env("MY_CONFIG").unwrap();
```

<hr>

Nesting fields:

```
use derive_environment::FromEnv;

#[derive(FromEnv)]
struct ServerConfig {
    port: u16,
}

#[derive(FromEnv)]
pub struct Config {
    server: ServerConfig,
}
```

Generates:
- MY_CONFIG_SERVER_PORT

<hr>

Vector of Nested fields:

```
use derive_environment::FromEnv;

// `Vec`'s `FromEnv` implementation requires `Default`.
#[derive(Default, FromEnv)]
struct ServerConfig {
    port: u16,
}

#[derive(FromEnv)]
pub struct Config {
    servers: Vec<ServerConfig>,
}
```

Generates:
- MY_CONFIG_SERVER_0_PORT
