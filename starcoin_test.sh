#!/bin/bash

cargo test -p diem-crypto test_serialize_key
cargo test -p diem-crypto test_serialize
cargo test -p move-vm-integration-tests readonly_func_call
cargo test -p move-vm-natives test_type_params_formatting
cargo test -p move-core-types test_serialize
cargo test -p move-core-types tests_parse_type_tag
cargo test -p move-core-types test_transaction_argument_display
