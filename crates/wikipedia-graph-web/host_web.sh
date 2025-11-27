#!/bin/bash
set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd $script_path

HOST="127.0.0.1"
SERVER_DIRECTORY="./server"
PORT=7878

while test $# -gt 0; do
  case "$1" in
    --v6)
      HOST="::1"
      ;;
    *)
      HOST="$1"
      break
      ;;
  esac
done

cargo install static-web-server

echo "starting server…"
echo "serving at $HOST:${PORT}"

(static-web-server --host $HOST --port $PORT --root $SERVER_DIRECTORY)
