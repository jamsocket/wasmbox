#!/bin/sh

set -e

export QUICKJS_WASM_SYS_WASI_SDK_PATH=/home/paul/wasi-sdk-14.0

cargo build --release --target=wasm32-wasi --manifest-path=$1/Cargo.toml

cargo run -p wasmbox-cli -- compile $1/target/wasm32-wasi/release/$1_example.wasm $1_example.bin
