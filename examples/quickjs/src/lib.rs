use quickjs_wasm_rs::Context;

#[allow(unused_imports)]
use wasmbox::prelude::*;

#[derive(Clone)]
struct IgnoreSend<T>(pub T);
unsafe impl<T> Send for IgnoreSend<T> {}
unsafe impl<T> Sync for IgnoreSend<T> {}

#[wasmbox]
async fn run(ctx: WasmBoxContext<Self>) {
    let context = IgnoreSend(Context::new().unwrap());

    loop {
        let message = ctx.next().await;
        let value = context.0.eval(&message).unwrap();

        ctx.send(format!("result: {:?}", value));
    }
}
