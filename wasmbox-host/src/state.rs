use ambient_authority::ambient_authority;
use cap_primitives::time::{Instant, SystemClock, SystemTime};
use rand_chacha::ChaCha12Rng;
use rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use wasi_common::{WasiClocks, WasiCtx, WasiSystemClock};
use wasmtime_wasi::WasiCtxBuilder;

const MUTEX_ERROR: &str = "Something panicked while holding the mutex, so we can't safely resume.";

pub struct WasmBoxState {
    time: Arc<AtomicU64>,
    rng: DummyRng,
}

#[derive(Clone)]
struct DummyRng {
    inner_rng: Arc<Mutex<ChaCha12Rng>>,
}

impl RngCore for DummyRng {
    fn next_u32(&mut self) -> u32 {
        self.inner_rng.lock().expect(MUTEX_ERROR).next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.inner_rng.lock().expect(MUTEX_ERROR).next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.inner_rng.lock().expect(MUTEX_ERROR).fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.inner_rng
            .lock()
            .expect(MUTEX_ERROR)
            .try_fill_bytes(dest)
    }
}

#[derive(Serialize, Deserialize)]
pub struct WasmBoxStateSnapshot {
    time: u64,
    rng: ChaCha12Rng,
}

impl WasmBoxState {
    pub fn new() -> WasmBoxState {
        let rng = ChaCha12Rng::from_seed([
            228, 89, 231, 220, 224, 20, 162, 27, 133, 157, 88, 214, 45, 102, 132, 24, 70, 0, 72,
            252, 102, 134, 132, 205, 244, 168, 130, 198, 122, 100, 17, 29,
        ]);

        WasmBoxState {
            time: Arc::default(),
            rng: DummyRng {
                inner_rng: Arc::new(Mutex::new(rng)),
            },
        }
    }

    pub fn wasi_ctx(&self) -> WasiCtx {
        let mut wasi = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build();

        // guaranteed to be random. https://xkcd.com/221/

        wasi.random = Box::new(self.rng.clone());

        wasi.clocks = WasiClocks {
            system: Box::new(FakeSystemClock::new(self.time.clone())),
            monotonic: Box::new(wasmtime_wasi::sync::clocks::MonotonicClock::new(
                ambient_authority(),
            )),
            creation_time: Instant::from_std(std::time::Instant::now()),
        };

        wasi
    }

    pub fn snapshot(&self) -> WasmBoxStateSnapshot {
        WasmBoxStateSnapshot {
            time: self.time.load(Ordering::Relaxed),
            rng: self.rng.inner_rng.lock().expect(MUTEX_ERROR).clone(),
        }
    }

    pub fn load_snapshot(&self, snapshot: &WasmBoxStateSnapshot) {
        self.time.store(snapshot.time, Ordering::Relaxed);
        *self.rng.inner_rng.lock().expect(MUTEX_ERROR) = snapshot.rng.clone();
    }

    pub fn set_time(&mut self, time: u64) {
        self.time.store(time, Ordering::Relaxed);
    }
}

pub struct FakeSystemClock {
    time: Arc<AtomicU64>,
}

impl FakeSystemClock {
    pub fn new(time: Arc<AtomicU64>) -> Self {
        FakeSystemClock { time }
    }
}

impl WasiSystemClock for FakeSystemClock {
    fn resolution(&self) -> std::time::Duration {
        Duration::from_millis(1)
    }

    fn now(&self, _precision: std::time::Duration) -> SystemTime {
        let time = self.time.load(Ordering::Relaxed);

        SystemClock::UNIX_EPOCH
            .checked_add(Duration::from_millis(time))
            .expect("Error creating time.")
    }
}
