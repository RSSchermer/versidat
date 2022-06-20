use std::mem;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

type Shared<T> = Arc<Mutex<Option<NonNull<ListenerInternal<T>>>>>;

pub struct Listener<T> {
    internal: Box<ListenerInternal<T>>,
}

impl<T> Deref for Listener<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.internal.value
    }
}

struct ListenerInternal<T> {
    value: T,
    previous: Option<NonNull<ListenerInternal<T>>>,
    next: Option<NonNull<ListenerInternal<T>>>,
    shared: Shared<T>,
}

impl<T> ListenerInternal<T> {
    fn unlink(&mut self) {
        let ListenerInternal {
            previous,
            next,
            shared,
            ..
        } = self;

        let mut lock = shared.lock().unwrap();

        if let Some(mut next) = next {
            unsafe {
                next.as_mut().previous = *previous;
            }
        }

        if let Some(mut previous) = previous {
            unsafe {
                previous.as_mut().next = *next;
            }
        } else {
            *lock = *next;
        }

        mem::drop(lock);
    }
}

impl<T> Drop for ListenerInternal<T> {
    fn drop(&mut self) {
        self.unlink();
    }
}

pub struct Broadcaster<T> {
    shared: Shared<T>,
}

impl<T> Broadcaster<T> {
    pub fn new() -> Self {
        Broadcaster {
            shared: Arc::new(Mutex::new(None)),
        }
    }

    pub fn broadcast<F>(&self, f: F)
    where
        F: Fn(&T),
    {
        let lock = self.shared.lock().unwrap();

        let mut next = *lock;

        while let Some(listener) = next {
            unsafe {
                let listener = listener.as_ref();

                f(&listener.value);

                next = listener.next;
            }
        }

        mem::drop(lock);
    }

    pub fn listener(&self, value: T) -> Listener<T> {
        let mut lock = self.shared.lock().unwrap();

        let listener = Box::new(ListenerInternal {
            value,
            previous: None,
            next: *lock,
            shared: self.shared.clone(),
        });

        let listener_ptr = NonNull::from(&*listener);

        if let Some(mut next) = *lock {
            unsafe {
                next.as_mut().previous = Some(listener_ptr);
            }
        }

        *lock = Some(listener_ptr);

        Listener { internal: listener }
    }
}

unsafe impl<T> Send for Broadcaster<T> {}
unsafe impl<T> Sync for Broadcaster<T> {}
