use quickjs_wasm_rs::Context;
use wasmbox::prelude::*;

#[derive(Clone)]
struct IgnoreSend<T>(pub T);
unsafe impl<T> Send for IgnoreSend<T> {}
unsafe impl<T> Sync for IgnoreSend<T> {}

#[wasmbox]
async fn run(ctx: WasmBoxContext<String, String>) {
    let context = IgnoreSend(Context::default());

    ctx.send("ready".to_string());
    loop {
        let message = ctx.next().await;
        match context.0.eval_global("main", &message) {
            Ok(value) => ctx.send(format!("=> {}", value.as_str().unwrap())),
            Err(e) => ctx.send(format!("ERROR: {:?}", e)),
        }
    }
}
