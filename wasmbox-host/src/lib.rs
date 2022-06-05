use anyhow::anyhow;
use serde::Deserialize;
use serde::{de::DeserializeOwned, Serialize};
use state::{WasmBoxState, WasmBoxStateSnapshot};
use std::fs::File;
use std::io::Write;
use std::marker::PhantomData;
use wasmtime::{Caller, Engine, Extern, Linker, Memory, Module, Store, TypedFunc};
use wasmtime_wasi::WasiCtx;

mod state;

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
) -> anyhow::Result<R>
where
    R: DeserializeOwned,
{
    let data = get_u8_vec(caller, memory, start, len);
    Ok(bincode::deserialize(data)?)
}

pub fn prepare_module(input_path: &str, output_path: &str) -> anyhow::Result<()> {
    let input_module = std::fs::read(input_path)?;
    let engine = Engine::default();

    let result = engine.precompile_module(&input_module)?;
    std::fs::write(output_path, &result)?;

    Ok(())
}

pub struct WasmBoxHost<Input: Serialize, Output: DeserializeOwned> {
    store: Store<WasiCtx>,
    memory: Memory,
    state: WasmBoxState,

    fn_malloc: TypedFunc<u32, u32>,
    fn_free: TypedFunc<(u32, u32), ()>,
    fn_send: TypedFunc<(u32, u32), ()>,

    _ph_i: PhantomData<Input>,
    _ph_o: PhantomData<Output>,
}

impl<Input: Serialize, Output: DeserializeOwned> WasmBoxHost<Input, Output> {
    fn put_data(&mut self, data: &[u8]) -> anyhow::Result<(u32, u32)> {
        #[allow(clippy::cast_possible_truncation)]
        let len = data.len() as u32;
        let pt = self.fn_malloc.call(&mut self.store, len)?;

        self.memory.write(&mut self.store, pt as usize, data)?;

        Ok((pt, len))
    }

    fn try_send(&mut self, message: &Input) -> anyhow::Result<()> {
        let (pt, len) = self.put_data(&bincode::serialize(message)?)?;

        self.fn_send.call(&mut self.store, (pt, len))?;

        self.fn_free.call(&mut self.store, (pt, len))?;
        Ok(())
    }

    pub fn from_compiled_module<F>(module_file: &str, callback: F) -> anyhow::Result<Self>
    where
        F: Fn(Output) + 'static + Send + Sync,
        Self: Sized,
    {
        let engine = Engine::default();
        let module = unsafe { Module::deserialize_file(&engine, module_file)? };

        Ok(Self::init(engine, module, callback)?)
    }

    pub fn from_wasm_file<F>(module_file: &str, callback: F) -> anyhow::Result<Self>
    where
        F: Fn(Output) + 'static + Send + Sync,
        Self: Sized,
    {
        let engine = Engine::default();
        let module = Module::from_file(&engine, module_file)?;

        Ok(Self::init(engine, module, callback)?)
    }

    fn init<F>(engine: Engine, module: Module, callback: F) -> anyhow::Result<Self>
    where
        F: Fn(Output) + 'static + Send + Sync,
        Self: Sized,
    {
        let state = WasmBoxState::new();
        let wasi = state.wasi_ctx();

        let mut store = Store::new(&engine, wasi);
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

        {
            linker.func_wrap(
                ENV,
                EXT_FN_CALLBACK,
                move |mut caller: Caller<'_, WasiCtx>, start: u32, len: u32| {
                    let memory = get_memory(&mut caller);
                    let message: Output = get_deserialize(&caller, &memory, start, len)?;

                    callback(message);
                    Ok(())
                },
            )?;
        }

        let instance = linker.instantiate(&mut store, &module)?;

        let memory = instance
            .get_memory(&mut store, EXT_MEMORY)
            .ok_or_else(|| anyhow!("Couldn't allocate memory."))?;

        let fn_malloc = instance.get_typed_func::<u32, u32, _>(&mut store, EXT_FN_MALLOC)?;
        let fn_free = instance.get_typed_func::<(u32, u32), (), _>(&mut store, EXT_FN_FREE)?;
        let fn_send = instance.get_typed_func::<(u32, u32), (), _>(&mut store, EXT_FN_SEND)?;
        let fn_initialize = instance.get_typed_func::<(), (), _>(&mut store, EXT_FN_INITIALIZE)?;

        fn_initialize.call(&mut store, ())?;

        Ok(WasmBoxHost {
            store,
            memory,
            state,
            fn_malloc,
            fn_free,
            fn_send,
            _ph_i: PhantomData::default(),
            _ph_o: PhantomData::default(),
        })
    }

    pub fn set_time(&mut self, time: u64) {
        self.state.set_time(time)
    }

    pub fn message(&mut self, input: &Input) {
        self.try_send(input).expect("Error sending message.")
    }

    pub fn snapshot_state(&self) -> anyhow::Result<Snapshot> {
        let memory = self.memory.data(&self.store);

        Ok(Snapshot {
            memory: memory.to_vec(),
            state: self.state.snapshot(),
        })
    }

    pub fn snapshot_to_file(&self, filename: &str) -> anyhow::Result<()> {
        let snapshot = self.snapshot_state()?;
        std::fs::write(filename, bincode::serialize(&snapshot)?)?;

        Ok(())
    }

    pub fn restore_snapshot(&mut self, snapshot: &Snapshot) -> anyhow::Result<()> {
        let mut p = self.memory.data_mut(&mut self.store);
        p.write_all(&snapshot.memory)?;

        self.state.load_snapshot(&snapshot.state);

        Ok(())
    }

    pub fn restore_snapshot_from_file(&mut self, filename: &str) -> anyhow::Result<()> {
        let contents: Snapshot = bincode::deserialize_from(File::open(filename)?)?;
        self.restore_snapshot(&contents)?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    memory: Vec<u8>,
    state: WasmBoxStateSnapshot,
}
