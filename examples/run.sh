#!/bin/sh

set -e

cargo run -p wasmbox-cli -- run $1_example.bin --freeze-time

