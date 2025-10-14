#!/bin/bash

current_path=$(basename "$PWD")

case "$current_path" in 
  "wikipedia-graph")
    echo "> Directory confirmed"
    ;;
  *)
    echo "Run this script in the workspace root"
    exit 1
esac

cargo run --bin wikimedia-language-codegen

cp "./crates/wikimedia-language-codegen/output/wikimedia_languages.rs" "./crates/wikipedia-graph/src/generated"
