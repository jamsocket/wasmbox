use async_trait::async_trait;
use wasmbox::{AsyncWasmBox, WasmBoxContext};

struct Echo;

#[async_trait]
impl AsyncWasmBox for Echo {
    type Input = String;
    type Output = String;

    async fn run(ctx: WasmBoxContext<Self>) -> ()
    {
        loop {
            let message = ctx.next().await;
            ctx.send(format!("Echo: {}", message));
        }        
    }
}
