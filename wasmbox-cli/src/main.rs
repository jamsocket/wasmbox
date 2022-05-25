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
                let line = line?;

                if line == "!!freeze" {
                    let timestamp = std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).expect("duration_should failed.");
                    let filename = format!("snapshot-{}.bin", timestamp.as_secs());

                    mybox.freeze(&filename)?;
                    println!("Froze to {}", filename);
                } else if let Some(filename) = line.strip_prefix("!!restore ") {
                    mybox.restore(&filename)?;
                    println!("Restored from {}", filename);
                } else {
                    mybox.message(line);
                }

                
            }
        }
    }

    Ok(())
}
