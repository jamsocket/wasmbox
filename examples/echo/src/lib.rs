use wasmbox::WasmBox;

struct Echo;

impl WasmBox for Echo {
    type Input = String;
    type Output = String;

    fn message<F>(&mut self, input: Self::Input, callback: F) where F: Fn(Self::Output) {
        callback(format!("echo: {}", input));
    }
}
