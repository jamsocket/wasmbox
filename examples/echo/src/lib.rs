#[allow(unused_imports)]
use wasmbox::wasm::*;
use wasmbox::{WasmBox};

struct Echo {
    callback: Box<dyn Fn(String)>,
}

// #[async_trait]
// impl AsyncWasmBox for Echo {
//     type Input = String;
//     type Output = String;

//     async fn run(ctx: WasmBoxContext<Self>) -> () {
//         loop {
//             let message = ctx.next().await;
//             ctx.send(format!("Echo: {}", message));
//         }
//     }
// }

impl WasmBox for Echo {
    type Input = String;
    type Output = String;

    fn init<F>(callback: F) -> Self
    where
        F: Fn(Self::Output) + 'static + Send + Sync, Self: Sized {
        
        Echo { callback: Box::new(callback) }
    }

    fn message(&mut self, input: Self::Input) {
        (self.callback)(format!("Echo: `{}`", input));
    }
}

#[no_mangle]
extern "C" fn initialize() {
    wasmbox::wasm::initialize::<Echo>()
}
