#!/bin/sh

# Sets some environment variables to demonstrate usage.rs
# Make sure to execute this as `. usage.sh` or `source usage.sh` rather than `sh usage.sh`


#   parseable: String,
#   #[env(ignore)]
#   ignored: UnparsableStruct,
#   #[env(nested)]
#   nested: SubStruct,
#   vector: Vec<String>,
#   #[env(nested)]
#   nested_vector: Vec<SubStruct>,
#   optional: Option<String>,

export TEST_PREFIX_PARSEABLE=Name
export TEST_PREFIX_NESTED__VALUE=80
export TEST_PREFIX_VECTOR__0=0
export TEST_PREFIX_VECTOR__1=1
export TEST_PREFIX_VECTOR__2=2
export TEST_PREFIX_VECTOR__3=3
export TEST_PREFIX_NESTED_VECTOR__0__VALUE=80
export TEST_PREFIX_NESTED_VECTOR__1__VALUE=22
export TEST_PREFIX_NESTED_VECTOR__2__VALUE=443
export TEST_PREFIX_OPTIONAL=Something
