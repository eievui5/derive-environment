use crate::FromEnv;
use encoding_rs::Encoding;
use std::env;

impl FromEnv for &'static Encoding {
	fn with_env(&mut self, s: &str) -> crate::Result<bool> {
		match env::var(s) {
			Ok(var) => {
				*self =
					Encoding::for_label(var.as_bytes()).ok_or(crate::FromEnvError::ParseError(
						s.to_string(),
						String::from("Unrecognized encoding"),
					))?;
				Ok(true)
			}
			Err(env::VarError::NotPresent) => Ok(false),
			Err(env::VarError::NotUnicode(os)) => {
				Err(crate::FromEnvError::NotUnicode(s.to_string(), os))
			}
		}
	}
}
