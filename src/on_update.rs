use crate::broadcast::{Broadcaster, Listener};
use futures::Stream;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};

struct Waiter {
    terminated: bool,
    waker: Option<Waker>,
}

pub struct UpdateBroadcaster {
    inner: Arc<Broadcaster<Mutex<Waiter>>>,
}

impl UpdateBroadcaster {
    pub fn new() -> Self {
        UpdateBroadcaster {
            inner: Arc::new(Broadcaster::new()),
        }
    }

    pub fn broadcast(&self) {
        self.inner.broadcast(|waiter| {
            let mut waiter = waiter.lock().unwrap();

            if let Some(waker) = waiter.waker.take() {
                waker.wake();
            }
        })
    }

    pub fn listener(&self) -> OnUpdate {
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
