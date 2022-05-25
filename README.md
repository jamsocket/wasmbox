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

#### Compiling guest modules

Guest modules should have the following in their `Cargo.toml`:

```text
[lib]
crate-type = ["cdylib", "rlib"]
```

They should be compiled with the target `wasm32-wasi` like so:

```text
cargo build --release --target=wasm32-wasi
```

You might have to install the `wasm32-wasi` target (e.g. using `rustup`).

Look for a `.wasm` file under `target/wasm32-wasi/release`.

### Host environment

The host environment is always the same (synchronous) interface, regardless of whether the guest module is using the asynchronous or synchronous interface.

Constructing a host environment (`WasmBoxHost`) requires two things: the module to load, and a callback to use for receiving messages from the guest module. The module can either be passed in as a `.wasm` file, or as a pre-compiled module.

See `wasmbox-cli` for an example of implementing a host environment.

```rust,no_run
use wasmbox_host::WasmBoxHost;
use anyhow::Result;

fn main() -> Result<()> {
    let mut mybox = WasmBoxHost::from_wasm_file("path/to/some/module.wasm",
        |st: String| println!("guest module says: {}", st))?;

    // Send some messages into the box:
    mybox.message("The guest module will receive this message.");
    mybox.message("And this one.");

    // Turn the state into a serializable object.
    let state = mybox.snapshot_state()?;
    
    // Or, serialize directly to disk:
    mybox.snapshot_to_file("snapshot.bin")?;

    // We can interact more with the box:
    mybox.message("Pretend this message has a side-effect on the box's state.");

    // And then restore the state, undoing the last side-effect.
    mybox.restore_snapshot(&state)?;

    // Or, restore directly from disk:
    mybox.restore_snapshot_from_file("snapshot.bin")?;

    Ok(())
}
```

### Synchronous Guest Interface

Rather than writing an async function to implement a guest, you can implement a `trait` and use the `#[wasmbox_sync]` macro.

Each WasmBox is constructed with a call to `init`. Each message from the host is passed through a call to the trait's `message` function. To pass messages back to the host, a boxed `callback` function is provided in `init`.

Both the `init` function and `message` functions are allowed to call the callback, and may do so multiple times.
In order to call the callback from `message`, you can store it in the type itself.

```rust,no_run
use wasmbox::prelude::*;

#[wasmbox_sync]
struct Counter {
    count: u32,
    callback: Box<dyn Fn(String) + Send + Sync>,
}

impl WasmBox for Counter {
    type Input = String;
    type Output = String;

    fn init(callback: Box<dyn Fn(Self::Output) + Send + Sync>) -> Self
    where
        Self: Sized,
    {
        Counter { count: 0, callback }
    }

    fn message(&mut self, input: Self::Input) {
        match input.as_ref() {
            "up" => self.count += 1,
            "down" => self.count -= 1,
            _ => return
        }

        (self.callback)(format!("value={}", self.count));
    }
}
```

## Safety

This module uses unsafe a lot, in particular within the WASM code. The host also uses unsafe when loading a pre-compiled module, which can lead to arbitrary code execution. Pre-compiled modules are safe **only** if you can be sure that they were created by wasmtime/cranelift.

## Limitations

- It's likely to be slower than native code, because it uses WebAssembly.
- To provide a deterministic environment, access to anything outside the sandbox is blocked. The system clock is mocked to create a deterministic (but monotonically increasing) clock. Random entropy is not random, but comes from a seeded pseudo-random number generator.
- To avoid unnecessary repetition, the state does not include the program module itself; it is up to the caller to ensure that the same WASM module that created a snapshot is running when the snapshot is restored.
- Currently, the `WasmBoxHost` environment owns *everything* about the WebAssembly environment, including things which could be shared between instances. This is inefficient if you want to run many instances of the same module, for instance.
- Probably lots of other things.
