#!/bin/bash

# A modified version of the script found in the ehttp examples
# https://github.com/emilk/ehttp/blob/master/example_eframe/build_web.sh

set -eu

# cd to 'wikipedia-graph-web'
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )

CRATE_NAME="wikipedia-graph-web"

OPTIMIZE=false
BUILD=debug
BUILD_FLAGS=""
WASM_OPT_FLAGS="-O2 --fast-math"

while test $# -gt 0; do
  case "$1" in
    -h|--help)
      echo "build_web.sh [--release]"
      echo "  --release: Build with --release, and then run wasm-opt."
      exit 0
      ;;

    --release)
      shift
      OPTIMIZE=true
      BUILD="release"
      BUILD_FLAGS="--release"
      ;;

    *)
      break
      ;;
  esac
done

SERVER_DIRECTORY="./server"

# Clear the server
rm -rf $SERVER_DIRECTORY

mkdir $SERVER_DIRECTORY

cp index.html  $SERVER_DIRECTORY

FINAL_WASM_PATH="$SERVER_DIRECTORY/wikipedia_graph_web_bg.wasm"

echo "Building rust…"

cargo build \
  ${BUILD_FLAGS} \
  --lib \
  --target wasm32-unknown-unknown

echo "Generating JS bindings for wasm…"

TARGET_NAME="wikipedia_graph_web.wasm"
wasm-bindgen "../../target/wasm32-unknown-unknown/$BUILD/$TARGET_NAME" \
  --out-dir $SERVER_DIRECTORY --no-modules --no-typescript

if [[ "${OPTIMIZE}" = true ]]; then
  echo "Optimizing wasm…"
  # to get wasm-opt:  apt/brew/dnf install binaryen
  wasm-opt "${FINAL_WASM_PATH}" $WASM_OPT_FLAGS -o "${FINAL_WASM_PATH}"
fi

echo "Finished ${FINAL_WASM_PATH}";
