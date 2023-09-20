#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub use derive_environment_macros::FromEnv;
use std::{ffi::OsString, path::PathBuf, str::FromStr};

/// Errors generated when populating a structure from the environment.
///
/// A missing environment variable is *not* considered an Error.
#[derive(Clone, Debug, thiserror::Error)]
pub enum FromEnvError {
	/// Thrown when an environment variable was found, but was not valid unicode.
	#[error("environment variable {0} was not valid unicode: {1:?}")]
	NotUnicode(String, OsString),
	/// Thrown when a unicode environment variable was found, but it could not be parsed.
	#[error("failed to parse environment variable {0}: {1}")]
	ParseError(String, String),
}

/// The result of [`FromEnv::with_env`].
pub type Result<T> = std::result::Result<T, FromEnvError>;

/// Denotes a type that may be read from an environment variable.
pub trait FromEnv: Sized {
	/// Reads and parses an environment variable.
	/// Returns `Ok(true)` if an environment variable was found and used, and `Ok(false)` if it was absent.
	///
	/// # Errors
	///
	/// Throws an error if the environment variable could not be read or parsed;
	fn with_env(&mut self, var: &str) -> Result<bool>;
}

/// Helper type for mainting a no-alloc string representation.
#[derive(Debug)]
struct DigitContainer {
	// This could be improved slightly by using `ascii::Char` with `from_u8_unchecked`,
	// but at the time of writing this requires nightly (and the obvious `unsafe` block).
	digits: [u8; usize::MAX.ilog10() as usize],
}

impl DigitContainer {
	fn new() -> Self {
		let mut digits: [u8; usize::MAX.ilog10() as usize] = [0; usize::MAX.ilog10() as usize];
		digits[0] = 1;
		Self { digits }
	}

	fn next(&mut self, s: &mut String) {
		let mut digit_iter = self.digits.into_iter().rev();

		while let Some(digit) = digit_iter.next() {
			if digit != 0 {
				s.push((digit + b'0').into());

				for digit in digit_iter {
					s.push((digit + b'0').into());
				}
				break;
			}
		}

		for digit in &mut self.digits {
			*digit += 1;
			if *digit != 10 {
				break;
			} else {
				*digit = 0
			}
		}
	}
}

/// Automatically implements [`FromEnv`] using the type's [`FromStr`] implementation.
#[macro_export]
macro_rules! impl_using_from_str {
    ($type:ty) => {
        impl FromEnv for $type {
            fn with_env(&mut self, var: &str) -> ::std::result::Result<bool, FromEnvError> {
                use std::env;

            	match ::std::env::var(var) {
            		Ok(s) => {
                        *self = s.parse().map_err(|msg: <$type as FromStr>::Err| FromEnvError::ParseError(var.to_string(), msg.to_string()))?;
                        Ok(true)
                    }
            		Err(env::VarError::NotPresent) => Ok(false),
            		Err(env::VarError::NotUnicode(s)) => Err(FromEnvError::NotUnicode(var.to_string(), s)),
            	}
            }
        }
    };
    ($($type:ty),+$(,)?) => {
		$(
			impl_using_from_str!($type);
		)+
    };
}

impl_using_from_str! {
	u8, u16, u32, u64, u128,
	i8, i16, i32, i64, i128,
	bool, String, PathBuf,
}

impl<T: FromEnv + Default> FromEnv for Option<T> {
	fn with_env(&mut self, var: &str) -> Result<bool> {
		let mut contents = T::default();

		let result = contents.with_env(var);

		if matches!(result, Ok(true)) {
			*self = Some(contents);
		}

		result
	}
}

impl<T: FromEnv + Default> FromEnv for Vec<T> {
	fn with_env(&mut self, prefix: &str) -> Result<bool> {
		// Working environment variable.
		let mut var = format!("{prefix}_0");

		// Special-case first element; if this is present, so is the vector.
		let mut contents = T::default();
		let mut v = if contents.with_env(&var)? {
			vec![contents]
		} else {
			return Ok(false);
		};

		// Counter as a string.
		// This is done on the stack to avoid allocations.
		let mut digits = DigitContainer::new();

		loop {
			// Rebuild var with no allocations.
			// (This isn't actually realloc-free; the string may overflow if the digit value becomes too large).
			// Truncate only modifies the "size" field, meaning we keep our allocated memory.
			var.truncate(prefix.len() + 1);
			// Then the previous digits are overwritten.
			digits.next(&mut var);

			let mut contents = T::default();
			if contents.with_env(&var)? {
				v.push(contents);
			} else {
				break;
			}
		}

		*self = v;

		Ok(true)
	}
}
