# WasmBox

[![GitHub Repo stars](https://img.shields.io/github/stars/drifting-in-space/wasmbox?style=social)](https://github.com/drifting-in-space/wasmbox)
[![crates.io](https://img.shields.io/crates/v/wasmbox.svg)](https://crates.io/crates/wasmbox)
[![docs.rs](https://img.shields.io/badge/docs-release-brightgreen)](https://docs.rs/wasmbox/)

WasmBox turns running Rust code into a serializable data structure.

It does this by compiling it to WebAssembly and running it in a sandbox. To snapshot the running code, it serializes the sandbox's linear memory, which contains the entire heap of the program.

**WasmBox is new and experimental.** Before relying on it in production code, feel free to open an issue and we can discuss 🙂.

## Interface

WasmBox has two components: the host environment and the guest module. The host environment is the program that interacts with the WasmBox from the outside. The guest module is the program that runs *inside* the WasmBox. The guest module is a separate Rust compiler artifact, compiled to target `wasm32-wasi`.

The two components interact through bidirectional, typed communication provided by WasmBox. Both synchronous and asynchronous interfaces are provided for developing the module.

To use the asynchronous interface, create a function with the signature `async fn run(ctx: WasmBoxContext<String, String>`, and decorate it with the `#[wasmbox]` annotation.

The following example implements a trivial stateful WasmBox guest module which stores counter state internally. It waits for input from the host environment. When it recieves the inputs `"up"` or `"down"` from the host environment, it modifies the counter state internally and publishes it back to the host environment.

```rust,no_run
#[allow(unused_imports)]
use wasmbox::prelude::*;

#[wasmbox]
async fn run(ctx: WasmBoxContext<String, String>) {
    let mut c = 0;
    loop {
        let message = ctx.next().await;
        match message.as_ref() {
            "up" => c += 1,
            "down" => c -= 1,
            _ => continue,
        }
        ctx.send(format!("value={}", c));
    }
}
```

Note: the `<String, String>` attributes of `WasmBoxContext` are the types of data passed into and out of the WasmBox, respectively. `ctx.next()` returns a value of the first type, and `ctx.send()` expects a value of the second type. If you are writing your own host environment, you can use any (de)serializable type here, as long as the pair of types is the same on both the host environment and the guest module. The demonstration host environment provided by `wasmbox-cli` only supports `<String, String>`, so that's what we use here.

### Host environment



## Safety

This module uses unsafe a lot, in particular within the WASM code. The host also uses unsafe when loading a pre-compiled module, which can lead to arbitrary code execution. Pre-compiled modules are safe **only** if you can be sure that they were created by wasmtime/cranelift.

## Limitations

- It's likely to be slower than native code, because it uses WebAssembly.
- To provide a deterministic environment, access to anything outside the sandbox is blocked. The system clock is mocked to create a deterministic (but monotonically increasing) clock. Random entropy is not random, but comes from a seeded pseudo-random number generator.
- To avoid unnecessary repetition, the state does not include the program module itself; it is up to the caller to ensure that the same WASM module that created a snapshot is running when the snapshot is restored.
- Probably lots of other things.
