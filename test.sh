#!/bin/sh

set -e

cargo build --target=wasm32-unknown-unknown -p $1_example

export WASMTIME_BACKTRACE_DETAILS=1

cargo run -p wasmbox-cli -- target/wasm32-unknown-unknown/debug/$1_example.wasm
