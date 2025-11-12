#!/bin/bash

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cargo run --bin wikimedia-language-codegen

cp "./crates/wikimedia-language-codegen/output/wikimedia_languages.rs" "./crates/wikipedia-graph/src/generated"
