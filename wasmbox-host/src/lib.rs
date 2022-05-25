use serde::{de::DeserializeOwned, Serialize};
use wasmtime::{Caller, Engine, Extern, Linker, Memory, Store, TypedFunc, Module};
use anyhow::anyhow;
use wasmtime_wasi::sync::WasiCtxBuilder;
use wasmtime_wasi::WasiCtx;

const ENV: &str = "env";
const EXT_MEMORY: &str = "memory";
const EXT_FN_CALLBACK: &str = "wasmbox_callback";
const EXT_FN_SEND: &str = "wasmbox_send";
const EXT_FN_MALLOC: &str = "wasmbox_malloc";
const EXT_FN_FREE: &str = "wasmbox_free";
const EXT_FN_INITIALIZE: &str = "wasmbox_initialize";

#[inline]
fn get_memory<T>(caller: &mut Caller<'_, T>) -> Memory {
    match caller.get_export(EXT_MEMORY) {
        Some(Extern::Memory(mem)) => mem,
        _ => panic!(),
    }
}

#[inline]
fn get_u8_vec<'a, T>(
    caller: &'a Caller<'_, T>,
    memory: &'a Memory,
    start: u32,
    len: u32,
) -> &'a [u8] {
    let data = memory
        .data(caller)
        .get(start as usize..(start + len) as usize);
    match data {
        Some(data) => data,
        None => panic!(),
    }
}

#[inline]
fn get_deserialize<'a, T, R>(
    caller: &'a Caller<'_, T>,
    memory: &'a Memory,
    start: u32,
    len: u32,
) -> anyhow::Result<R> where R: DeserializeOwned {
    let data = get_u8_vec(caller, memory, start, len);
    Ok(bincode::deserialize(data)?)
}

// TODO: use from wasmbox rather than re-implementing.
pub trait WasmBox: 'static {
    type Input: DeserializeOwned;
    type Output: Serialize;

    fn init<F>(wasm_file: &str, callback: F) -> anyhow::Result<Self>
    where
        F: Fn(Self::Output) + 'static + Send + Sync,
        Self: Sized;

    fn message(&mut self, input: Self::Input);
}

pub struct WasmBoxHost {
    store: Store<WasiCtx>,
    memory: Memory,

    fn_malloc: TypedFunc<u32, u32>,
    fn_free: TypedFunc<(u32, u32), ()>,
    fn_send: TypedFunc<(u32, u32), ()>,
}

impl WasmBoxHost {
    fn put_data(&mut self, data: &[u8]) -> anyhow::Result<(u32, u32)> {
        #[allow(clippy::cast_possible_truncation)]
        let len = data.len() as u32;
        let pt = self.fn_malloc.call(&mut self.store, len)?;

        self.memory.write(&mut self.store, pt as usize, data)?;

        Ok((pt, len))
    }

    fn try_send(&mut self, message: String) -> anyhow::Result<()> {
        let (pt, len) = self
            .put_data(&bincode::serialize(&message)?)?;

        self.fn_send
            .call(&mut self.store, (pt, len))?;

        self.fn_free
            .call(&mut self.store, (pt, len))?;
        Ok(())
    }
}

impl WasmBox for WasmBoxHost {
    type Input = String;
    type Output = String;

    fn init<F>(wasm_file: &str, callback: F) -> anyhow::Result<Self>
    where
        F: Fn(Self::Output) + 'static + Send + Sync,
        Self: Sized,
    {
        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_file)?;

        let wasi = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build();

        let mut store = Store::new(&engine, wasi);
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

        {
            linker.func_wrap(
                ENV,
                EXT_FN_CALLBACK,
                move |mut caller: Caller<'_, WasiCtx>, start: u32, len: u32| {
                    let memory = get_memory(&mut caller);
                    let message: String = get_deserialize(&caller, &memory, start, len)?;

                    callback(message.to_string());
                    Ok(())
                },
            )?;
        }

        let instance = linker.instantiate(&mut store, &module)?;

        let memory = instance
            .get_memory(&mut store, EXT_MEMORY).ok_or_else(|| anyhow!("Couldn't allocate memory."))?;

        let fn_malloc = instance.get_typed_func::<u32, u32, _>(&mut store, EXT_FN_MALLOC)?;
        let fn_free = instance.get_typed_func::<(u32, u32), (), _>(&mut store, EXT_FN_FREE)?;
        let fn_send =
            instance.get_typed_func::<(u32, u32), (), _>(&mut store, EXT_FN_SEND)?;
        let fn_initialize = instance.get_typed_func::<(), (), _>(&mut store, EXT_FN_INITIALIZE)?;

        fn_initialize.call(&mut store, ())?;

        Ok(WasmBoxHost {
            store,
            memory,
            fn_malloc,
            fn_free,
            fn_send,
        })
    }

    fn message(&mut self, input: Self::Input) {
        self.try_send(input).expect("Error sending message.")
    }
}
