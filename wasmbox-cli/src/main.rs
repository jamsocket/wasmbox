use anyhow::anyhow;
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
    /// Compile a wasm module to a preprocessed module. This is an optional step for faster start-up times.
    Compile {
        /// The path to a .wasm file.
        wasm_filename_in: String,

        /// The name of the file to write compiled data to.
        module_filename_out: String,
    },
    /// Run a module interactively.
    Run {
        /// The path to a compiled module (as output by the compile command.)
        compiled_module_filename: Option<String>,

        /// The path to a .wasm file (as output directly from the Rust compiler.)
        wasm_filename: Option<String>,
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
        Command::Run {
            compiled_module_filename,
            wasm_filename,
        } => {
            let mut mybox = if let Some(compiled_module_filename) = compiled_module_filename {
                WasmBoxHost::from_compiled_module(&compiled_module_filename, |st: String| {
                    println!("==> [{}]", st)
                })?
            } else if let Some(wasm_filename) = wasm_filename {
                WasmBoxHost::from_wasm_file(&wasm_filename, |st: String| println!("==> [{}]", st))?
            } else {
                return Err(anyhow!(
                    "Either --wasm-filename or --compiled-module-filename must be given."
                ));
            };

            let stdin = std::io::stdin();
            let iterator = stdin.lock().lines();

            for line in iterator {
                let line = line?;

                if line == "!!snapshot" {
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .expect("duration_should failed.");
                    let filename = format!("snapshot-{}.bin", timestamp.as_secs());

                    mybox.snapshot_to_file(&filename)?;
                    println!("Froze to {}", filename);
                } else if let Some(filename) = line.strip_prefix("!!restore ") {
                    mybox.restore_snapshot_from_file(&filename)?;
                    println!("Restored from {}", filename);
                } else {
                    mybox.message(line);
                }
            }
        }
    }

    Ok(())
}
