use wasmbox::prelude::*;

#[wasmbox]
async fn run(ctx: WasmBoxContext<String, String>) {
    let mut c = 0;
    loop {
        let message = ctx.next().await;
        match message.as_ref() {
            "up" => c += 1,
            "down" => c -= 1,
            _ => continue,
        }
        ctx.send(format!("value={}", c));
    }
}
