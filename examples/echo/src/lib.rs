#[allow(unused_imports)]
use wasmbox::wasm::*;
use wasmbox::{AsyncWasmBox, WasmBoxContext};
use async_trait::async_trait;

struct Echo;

#[async_trait]
impl AsyncWasmBox for Echo {
    type Input = String;
    type Output = String;

    async fn run(ctx: WasmBoxContext<Self>) -> () {
        loop {
            let message = ctx.next().await;
            ctx.send(format!("Echo: {}", message));
        }
    }
}

#[no_mangle]
extern "C" fn initialize() {
    wasmbox::wasm::initialize_async::<Echo>()
}
