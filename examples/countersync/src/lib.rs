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
