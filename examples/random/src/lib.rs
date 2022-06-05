use wasmbox::prelude::*;
use getrandom::getrandom;

#[wasmbox]
async fn run(ctx: WasmBoxContext<String, String>) {
    let mut c = 0;
    loop {
        let _ = ctx.next().await;
        
        let mut buf: [u8; 4] = [0; 4];
        getrandom(&mut buf).expect("Error calling getrandom.");
        let val: u32 = bytemuck::cast(buf);

        ctx.send(format!("random={}", val));
    }
}
