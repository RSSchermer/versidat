use std::cell::Cell;
use std::marker;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::task::{Context, Poll, Waker};

use atomic_counter::{AtomicCounter, RelaxedCounter};
use futures::Stream;
use lazy_static::lazy_static;

use crate::broadcast::{Broadcaster, Listener};
use crate::TypeConstructor;

lazy_static! {
    static ref STORE_ID_PROVIDER: RelaxedCounter = RelaxedCounter::new(0);
}

struct Shared<C>
where
    C: TypeConstructor,
{
    data: <C as TypeConstructor>::Type<'static>,
    update_context_provider: UpdateContextProvider,
}

struct Lock<C>
where
    C: TypeConstructor,
{
    shared: RwLock<Shared<C>>,
    store_id: usize,
}

impl<C> Lock<C>
where
    C: TypeConstructor,
{
    fn with<F, R>(&self, f: F) -> R
    where
        F: for<'store> FnOnce(&<C as TypeConstructor>::Type<'store>, ReadContext<'store>) -> R,
    {
        let lock = self.shared.read().expect("poisoned");

        unsafe {
            f(
                ::std::mem::transmute::<&<C as TypeConstructor>::Type<'static>, _>(&lock.data),
                ReadContext::new(self.store_id),
            )
        }
    }
}

pub struct Store<C>
where
    C: TypeConstructor,
{
    lock: Arc<Lock<C>>,
    update_broadcaster: UpdateBroadcaster,
}

impl<C> Store<C>
where
    C: TypeConstructor,
{
    pub fn initialize<F>(initializer: F) -> Self
    where
        F: for<'store> FnOnce(UpdateContext<'store>) -> <C as TypeConstructor>::Type<'store>,
    {
        let mut update_context_provider = UpdateContextProvider::new();

        let data = unsafe { initializer(update_context_provider.update_context()) };

        let shared = Shared {
            data,
            update_context_provider,
        };

        let store_id = STORE_ID_PROVIDER.inc();

        Store {
            lock: Arc::new(Lock {
                shared: RwLock::new(shared),
                store_id,
            }),
            update_broadcaster: UpdateBroadcaster::new(),
        }
    }

    pub fn id(&self) -> usize {
        self.lock.store_id
    }

    pub fn with<F, O>(&self, f: F) -> O
    where
        F: for<'store> FnOnce(&<C as TypeConstructor>::Type<'store>, ReadContext<'store>) -> O,
    {
        self.lock.with(f)
    }

    pub fn update<F>(&self, f: F)
    where
        F: for<'store> FnOnce(&<C as TypeConstructor>::Type<'store>, UpdateContext<'store>),
    {
        let mut lock = self.lock.shared.write().expect("poisoned");

        let Shared {
            data,
            update_context_provider,
        } = &mut *lock;

        let result = unsafe {
            f(
                ::std::mem::transmute::<&mut <C as TypeConstructor>::Type<'static>, _>(data),
                update_context_provider.update_context(),
            );
        };

        self.update_broadcaster.broadcast();

        result
    }

    /// Returns a stream that, once spawned, will be notified whenever an update scope for this
    /// store ends.
    pub fn on_update(&self) -> OnUpdate {
        self.update_broadcaster.listener()
    }
}

impl<C> Clone for Store<C>
where
    C: TypeConstructor,
{
    fn clone(&self) -> Self {
        Store {
            lock: self.lock.clone(),
            update_broadcaster: self.update_broadcaster.clone(),
        }
    }
}

struct Waiter {
    terminated: bool,
    waker: Option<Waker>,
}

#[derive(Clone)]
struct UpdateBroadcaster {
    inner: Arc<Broadcaster<Mutex<Waiter>>>,
}

impl UpdateBroadcaster {
    fn new() -> Self {
        UpdateBroadcaster {
            inner: Arc::new(Broadcaster::new()),
        }
    }

    fn broadcast(&self) {
        self.inner.broadcast(|waiter| {
            let mut waiter = waiter.lock().unwrap();

            if let Some(waker) = waiter.waker.take() {
                waker.wake();
            }
        })
    }

    fn listener(&self) -> OnUpdate {
        OnUpdate {
            broadcaster: Arc::downgrade(&self.inner),
            listener: None,
        }
    }
}

impl Drop for UpdateBroadcaster {
    fn drop(&mut self) {
        self.inner.broadcast(|waiter| {
            if let Ok(mut waiter) = waiter.lock() {
                waiter.terminated = true;

                if let Some(waker) = waiter.waker.take() {
                    waker.wake();
                }
            }
        })
    }
}

pub struct OnUpdate {
    broadcaster: Weak<Broadcaster<Mutex<Waiter>>>,
    listener: Option<Listener<Mutex<Waiter>>>,
}

impl Stream for OnUpdate {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.listener {
            None => {
                // Initialize if the broad caster is still alive, or terminate immediately
                if let Some(broadcaster) = self.broadcaster.upgrade() {
                    self.listener = Some(broadcaster.listener(Mutex::new(Waiter {
                        terminated: false,
                        waker: Some(cx.waker().clone()),
                    })));

                    Poll::Pending
                } else {
                    Poll::Ready(None)
                }
            }
            Some(listener) => {
                let mut waiter = listener.lock().unwrap();

                if waiter.terminated {
                    Poll::Ready(None)
                } else {
                    waiter.waker = Some(cx.waker().clone());

                    Poll::Ready(Some(()))
                }
            }
        }
    }
}

impl Clone for OnUpdate {
    fn clone(&self) -> Self {
        OnUpdate {
            broadcaster: self.broadcaster.clone(),
            listener: None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ReadContext<'store> {
    store_id: usize,
    _scope_marker: marker::PhantomData<Cell<&'store ()>>,
}

impl<'store> ReadContext<'store> {
    unsafe fn new(store_id: usize) -> ReadContext<'store> {
        ReadContext {
            store_id,
            _scope_marker: marker::PhantomData,
        }
    }

    pub fn store_id(&self) -> usize {
        self.store_id
    }
}

#[derive(Clone, Copy)]
pub struct UpdateContext<'store> {
    // Opting to use a raw pointer here rather than a reference or cell, so the context can by Copy.
    next_version: *mut u64,
    _scope_marker: marker::PhantomData<Cell<&'store ()>>,
}

impl UpdateContext<'_> {
    pub(crate) fn next_version(&self) -> u64 {
        // SAFETY: there is only ever a single update scope, and though there can be many
        // `UpdateContext`s within that scope (it implements `Copy`), `next_version` can never be
        // called concurrently
        unsafe {
            let next_version = *self.next_version;

            *self.next_version = next_version + 1;

            next_version
        }
    }
}

#[doc(hidden)]
struct UpdateContextProvider {
    next_version: u64,
}

impl UpdateContextProvider {
    #[doc(hidden)]
    fn new() -> Self {
        UpdateContextProvider { next_version: 0 }
    }

    #[doc(hidden)]
    unsafe fn update_context<'store>(&mut self) -> UpdateContext<'store> {
        UpdateContext {
            next_version: &mut self.next_version as *mut u64,
            _scope_marker: marker::PhantomData,
        }
    }
}
