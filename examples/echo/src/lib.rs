#[allow(unused_imports)]
use wasmbox::prelude::*;

#[wasmbox]
async fn run(ctx: WasmBoxContext<Self>) {
    loop {
        let message = ctx.next().await;
        ctx.send(format!("Echo: {}", message));
    }
}
