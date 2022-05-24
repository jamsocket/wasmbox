use clap::Parser;
use wasmbox_host::{WasmBox, WasmBoxHost};

#[derive(Parser)]
struct Opts {
    wasm_filename: String,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    let mut mybox =
        WasmBoxHost::init(&opts.wasm_filename, |st| println!("got: [{}]", st))?;

    mybox.message("heyo".to_string());

    Ok(())
}
