#!/bin/bash

current_path=$(basename "$PWD")

case "$current_path" in 
  "wikimedia-language-codegen")
    echo "> Directory confirmed"
    ;;
  *)
    echo "Run this script in the 'wikimedia-language-codegen' directory"
    exit 1
esac

cargo run

cp "./output/wikimedia_languages.rs" "../wikipedia-graph/src"
