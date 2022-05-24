use crate::WasmBox;

extern crate alloc;

static mut WASM_BOX: Option<Box<dyn WasmBox<Input=String, Output=String>>> = None;

extern "C" {
    /// Send a message from the wasm module to the host.
    pub fn callback(message_ptr: u32, message_len: u32);
}

pub fn wrapped_callback(message: String) {
    let message = bincode::serialize(&message).expect("Error serializing.");
    unsafe {
        callback(
            &message[0] as *const u8 as u32,
            message.len() as u32,
        );
    }
}

pub fn initialize<B>() where B: WasmBox<Input=String, Output=String> {
    let wasm_box = B::init(wrapped_callback);
    unsafe {
        WASM_BOX.replace(Box::new(wasm_box));
    }
}

#[no_mangle]
extern "C" fn send(ptr: *const u8, len: usize) {
    unsafe {
        let bytes = std::slice::from_raw_parts(ptr, len).to_vec();
        let message = bincode::deserialize(&bytes).expect("Error deserializing.");
        WASM_BOX.as_mut().expect("Received message before initialized.").message(message);
    };
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: u32) -> *mut u8 {
    let layout = core::alloc::Layout::from_size_align_unchecked(size as usize, 0);
    alloc::alloc::alloc(layout)
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut u8, size: u32) {
    let layout = core::alloc::Layout::from_size_align_unchecked(size as usize, 0);
    alloc::alloc::dealloc(ptr, layout);
}