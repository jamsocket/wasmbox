use clap::Parser;
use wasmbox_host::{WasmBox, WasmBoxHost};
use std::io::BufRead;

#[derive(Parser)]
struct Opts {
    wasm_filename: String,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    let mut mybox =
        WasmBoxHost::init(&opts.wasm_filename, |st| println!("==> [{}]", st))?;

    let stdin = std::io::stdin();
    let iterator = stdin.lock().lines();

    for line in iterator {
        mybox.message(line?);
    }

    Ok(())
}
