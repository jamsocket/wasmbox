use clap::{Parser, Subcommand};
use std::io::BufRead;
use wasmbox_host::{prepare_module, WasmBoxHost};

#[derive(Parser)]
struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Compile {
        wasm_filename_in: String,
        module_filename_out: String,
    },
    Run {
        module_filename: String,
    },
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    match opts.command {
        Command::Compile {
            wasm_filename_in,
            module_filename_out,
        } => {
            prepare_module(&wasm_filename_in, &module_filename_out)?;
        }
        Command::Run { module_filename } => {
            let mut mybox =
                WasmBoxHost::init(&module_filename, |st: String| println!("==> [{}]", st))?;

            let stdin = std::io::stdin();
            let iterator = stdin.lock().lines();

            for line in iterator {
                mybox.message(line?);
            }
        }
    }

    Ok(())
}
