#![doc = include_str!("../README.md")]

pub mod prelude;
pub mod wasm;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    sync::mpsc::{channel, Receiver, Sender, TryRecvError},
    task::{Context, Poll, Waker},
};

/// WASM is single-threaded, so we can safely ignore Send requirements.
#[derive(Clone)]
struct IgnoreSend<T>(pub T);
unsafe impl<T> Send for IgnoreSend<T> {}
unsafe impl<T> Sync for IgnoreSend<T> {}

pub trait WasmBox: 'static {
    type Input: Serialize;
    type Output: DeserializeOwned;

    fn init(callback: Box<dyn Fn(Self::Output) + Send + Sync>) -> Self
    where
        Self: Sized;

    fn message(&mut self, input: Self::Input);
}

pub struct NextMessageFuture<Input> {
    _ph_output: PhantomData<Input>,
    queue: IgnoreSend<Rc<Receiver<Input>>>,
}

impl<Input> Future for NextMessageFuture<Input> {
    type Output = Input;

    fn poll(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Input> {
        match self.queue.0.try_recv() {
            Ok(value) => Poll::Ready(value),
            Err(TryRecvError::Empty) => Poll::Pending,
            _ => panic!("Queue became disconnected."),
        }
    }
}

pub struct WasmBoxContext<Input, Output> {
    callback: Box<dyn Fn(Output) + Send + Sync>,
    queue: IgnoreSend<Rc<Receiver<Input>>>,
    _ph_o: PhantomData<Output>,
}

impl<Input, Output> WasmBoxContext<Input, Output> {
    fn new(callback: Box<dyn Fn(Output) + Send + Sync>, receiver: Receiver<Input>) -> Self {
        WasmBoxContext {
            callback,
            queue: IgnoreSend(Rc::new(receiver)),
            _ph_o: PhantomData::default(),
        }
    }

    pub fn send(&self, output: Output) {
        (self.callback)(output);
    }

    pub fn next(&self) -> NextMessageFuture<Input> {
        NextMessageFuture {
            _ph_output: PhantomData::default(),
            queue: self.queue.clone(),
        }
    }
}

#[async_trait]
pub trait AsyncWasmBox: 'static + Sized {
    type Input: Serialize;
    type Output: DeserializeOwned;

    async fn run(ctx: WasmBoxContext<Self::Input, Self::Output>) -> ();
}

mod dummy_context {
    use std::{
        ptr,
        task::{RawWaker, RawWakerVTable, Waker},
    };

    type WakerData = *const ();

    unsafe fn clone(_: WakerData) -> RawWaker {
        raw_waker()
    }
    unsafe fn wake(_: WakerData) {
        panic!("Should never wake dummy waker!")
    }
    unsafe fn wake_by_ref(_: WakerData) {
        panic!("Should never wake dummy waker!")
    }
    unsafe fn drop(_: WakerData) {}

    static MY_VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    fn raw_waker() -> RawWaker {
        RawWaker::new(ptr::null(), &MY_VTABLE)
    }

    pub fn waker() -> Waker {
        unsafe { Waker::from_raw(raw_waker()) }
    }
}

pub struct AsyncWasmBoxBox<B>
where
    B: AsyncWasmBox,
{
    future: Pin<Box<dyn Future<Output = ()>>>,
    sender: Sender<B::Input>,
    _ph_b: PhantomData<B>,
    waker: Waker,
}

impl<B> AsyncWasmBoxBox<B>
where
    B: AsyncWasmBox,
{
    fn poll(&mut self) {
        match self
            .future
            .as_mut()
            .poll(&mut Context::from_waker(&self.waker))
        {
            Poll::Ready(_) => panic!("Function exited."),
            Poll::Pending => (),
        }
    }
}

impl<B> WasmBox for AsyncWasmBoxBox<B>
where
    B: AsyncWasmBox,
{
    type Input = B::Input;
    type Output = B::Output;

    fn init(callback: Box<dyn Fn(B::Output) + Send + Sync>) -> Self {
        let (sender, recv) = channel();
        let ctx = WasmBoxContext::new(callback, recv);
        let future = B::run(ctx);
        let waker = dummy_context::waker();

        let mut async_box = AsyncWasmBoxBox {
            future,
            sender,
            waker,
            _ph_b: PhantomData::default(),
        };

        async_box.poll();
        async_box
    }

    fn message(&mut self, input: Self::Input) {
        self.sender.send(input).expect("Error sending message.");

        self.poll();
    }
}
