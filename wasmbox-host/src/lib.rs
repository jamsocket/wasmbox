use serde::{de::DeserializeOwned, Serialize};
use wasmtime::{Caller, Engine, Extern, Linker, Memory, Store, TypedFunc, Module};
use anyhow::anyhow;

const ENV: &str = "env";
const EXT_MEMORY: &str = "memory";
const EXT_FN_CALLBACK: &str = "callback";
const EXT_FN_SEND: &str = "send";
const EXT_FN_MALLOC: &str = "malloc";
const EXT_FN_FREE: &str = "free";


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
fn get_string<'a, T>(
    caller: &'a Caller<'_, T>,
    memory: &'a Memory,
    start: u32,
    len: u32,
) -> anyhow::Result<&'a str> {
    let data = get_u8_vec(caller, memory, start, len);
    Ok(std::str::from_utf8(data)?)
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

struct WasmBoxHost {
    store: Store<()>,
    memory: Memory,
    //callback: Box<dyn Fn(String)>,

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

        let mut store = Store::new(&engine, ());
        let mut linker = Linker::new(&engine);

        {
            linker.func_wrap(
                ENV,
                EXT_FN_SEND,
                move |mut caller: Caller<'_, ()>, start: u32, len: u32| {
                    let memory = get_memory(&mut caller);
                    let message = get_string(&caller, &memory, start, len)?;

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
            instance.get_typed_func::<(u32, u32), (), _>(&mut store, EXT_FN_CALLBACK)?;

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
