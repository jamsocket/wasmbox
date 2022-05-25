#!/bin/sh

set -e

export QUICKJS_WASM_SYS_WASI_SDK_PATH=/home/paul/wasi-sdk-14.0

cargo build --release --target=wasm32-wasi -p $1_example

#export WASMTIME_BACKTRACE_DETAILS=1

cargo run -p wasmbox-cli -- compile target/wasm32-wasi/release/$1_example.wasm $1_example.bin
