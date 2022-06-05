use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::{io::BufRead, time::SystemTime};
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

        /// If provided, turns automatic clock updates off. Time will only be updated when
        /// !!clock command is provided.
        #[clap(long)]
        freeze_time: bool,
    },
}

enum InteractiveCommand {
    SaveSnapshot,
    RestoreSnapshot(String),
    UpdateClock(Option<u64>),
    SendMessage(String),
}

impl InteractiveCommand {
    pub fn parse(line: &str) -> Result<InteractiveCommand> {
        if let Some(command_line) = line.strip_prefix("!!") {
            let mut command_parts = command_line.split_whitespace().into_iter();
            if let Some(command) = command_parts.next() {
                match command {
                    "snapshot" => Ok(InteractiveCommand::SaveSnapshot),
                    "restore" => Ok(InteractiveCommand::RestoreSnapshot(
                        command_parts
                            .next()
                            .ok_or_else(|| {
                                anyhow!("Filename to restore expected after !!restore.")
                            })?
                            .to_string(),
                    )),
                    "clock" => {
                        let time = if let Some(time) = command_parts.next() {
                            Some(time.parse()?)
                        } else {
                            None
                        };
                        Ok(InteractiveCommand::UpdateClock(time))
                    },
                    cmd => Err(anyhow!("Unknown command {}", cmd))
                }
            } else {
                return Err(anyhow!("Expected command to follow '!!'"));
            }
        } else {
            Ok(InteractiveCommand::SendMessage(line.to_string()))
        }
    }
}

fn current_time() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("Invalid system time.").as_millis() as u64
}

fn do_command(
    wasmbox: &mut WasmBoxHost<String, String>,
    command: &InteractiveCommand,
) -> Result<()> {
    match command {
        InteractiveCommand::SaveSnapshot => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("duration_should failed.");
            let filename = format!("snapshot-{}.bin", timestamp.as_secs());

            wasmbox.snapshot_to_file(&filename)?;
            println!("Froze to {}", filename);
        }
        InteractiveCommand::RestoreSnapshot(filename) => {
            wasmbox.restore_snapshot_from_file(&filename)?;
            println!("Restored from {}", filename);
        }
        InteractiveCommand::SendMessage(line) => {
            wasmbox.message(line);
        }
        InteractiveCommand::UpdateClock(time) => {
            let time = time.unwrap_or_else(current_time);

            wasmbox.set_time(time);
        }
    }

    Ok(())
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
            freeze_time,
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

                let command = match InteractiveCommand::parse(&line) {
                    Ok(command) => command,
                    Err(error) => {
                        println!("Error understanding command. {:?}", error);
                        continue
                    }
                };

                if !freeze_time {
                    mybox.set_time(current_time());
                }

                if let Err(error) = do_command(&mut mybox, &command) {
                    println!("Error running command. {:?}", error);
                }
            }
        }
    }

    Ok(())
}
