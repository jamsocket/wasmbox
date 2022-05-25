use ambient_authority::ambient_authority;
use anyhow::anyhow;
use cap_std::time::{Duration, Instant, SystemClock, SystemTime};
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha12Rng;
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use wasi_common::{WasiClocks, WasiSystemClock};
use wasmtime::{Caller, Engine, Extern, Linker, Memory, Module, Store, TypedFunc};
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

    fn_malloc: TypedFunc<u32, u32>,
    fn_free: TypedFunc<(u32, u32), ()>,
    fn_send: TypedFunc<(u32, u32), ()>,

    _ph_i: PhantomData<Input>,
    _ph_o: PhantomData<Output>,
}

struct FakeSystemClock {
    time: AtomicU64,
}

impl FakeSystemClock {
    pub fn new() -> Self {
        FakeSystemClock {
            time: AtomicU64::new(0),
        }
    }
}

impl WasiSystemClock for FakeSystemClock {
    fn resolution(&self) -> std::time::Duration {
        Duration::from_secs(1)
    }

    fn now(&self, _precision: std::time::Duration) -> SystemTime {
        let time = self.time.fetch_add(1, Ordering::Relaxed);

        SystemClock::UNIX_EPOCH
            .checked_add(Duration::from_secs(time))
            .expect("Error creating time.")
    }
}

fn dummy_wasi_clocks() -> WasiClocks {
    WasiClocks {
        system: Box::new(FakeSystemClock::new()),
        monotonic: Box::new(wasmtime_wasi::sync::clocks::MonotonicClock::new(
            ambient_authority(),
        )),
        creation_time: Instant::from_std(std::time::Instant::now()),
    }
}

impl<Input: Serialize, Output: DeserializeOwned> WasmBoxHost<Input, Output> {
    fn put_data(&mut self, data: &[u8]) -> anyhow::Result<(u32, u32)> {
        #[allow(clippy::cast_possible_truncation)]
        let len = data.len() as u32;
        let pt = self.fn_malloc.call(&mut self.store, len)?;

        self.memory.write(&mut self.store, pt as usize, data)?;

        Ok((pt, len))
    }

    fn try_send(&mut self, message: Input) -> anyhow::Result<()> {
        let (pt, len) = self.put_data(&bincode::serialize(&message)?)?;

        self.fn_send.call(&mut self.store, (pt, len))?;

        self.fn_free.call(&mut self.store, (pt, len))?;
        Ok(())
    }

    pub fn init<F>(module_file: &str, callback: F) -> anyhow::Result<Self>
    where
        F: Fn(Output) + 'static + Send + Sync,
        Self: Sized,
    {
        let engine = Engine::default();
        //let module = Module::from_file(&engine, wasm_file)?;
        let module = unsafe { Module::deserialize_file(&engine, module_file)? };

        let mut wasi = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build();

        let rng = ChaCha12Rng::from_seed([
            228, 89, 231, 220, 224, 20, 162, 27, 133, 157, 88, 214, 45, 102, 132, 24, 70, 0, 72,
            252, 102, 134, 132, 205, 244, 168, 130, 198, 122, 100, 17, 29,
        ]);
        wasi.random = Box::new(rng);

        let clocks = dummy_wasi_clocks();
        wasi.clocks = clocks;

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
            fn_malloc,
            fn_free,
            fn_send,
            _ph_i: PhantomData::default(),
            _ph_o: PhantomData::default(),
        })
    }

    pub fn message(&mut self, input: Input) {
        self.try_send(input).expect("Error sending message.")
    }
}
