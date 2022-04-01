use std::sync::{Arc, RwLock};
use std::marker;
use std::cell::Cell;
use crate::on_version_change::VersionChangeBroadcaster;

struct VersionProvider {
    next: u64
}

impl VersionProvider {
    fn next(&mut self) -> u64 {
        let next = self.next;

        self.next += 1;

        next
    }
}

struct StoreState<T> {
    data: T,
    version_provider: VersionProvider
}

pub struct UpdateContext<'store, 'context> {
    version_provider: &'context mut VersionProvider,
    _scope_marker: marker::PhantomData<Cell<&'store ()>>,
}

impl<'store, 'context> UpdateContext<'store, 'context> {
    fn new(version_provider: &'context mut VersionProvider) -> Self {
        UpdateContext {
            version_provider,
            _scope_marker: marker::PhantomData
        }
    }

    pub(crate) fn next_version(&self) -> u64 {
        self.version_provider.next()
    }
}

pub struct Store<T, R, E> {
    pub(crate) data: Arc<RwLock<StoreState<T>>>,
    reductor: R,
    _event_marker: marker::PhantomData<*const E>
}

impl<T, R, E> Store<T, R, E> where R: Fn(E, &mut T) {
    pub fn new<F>(initializer: F, reductor: R) -> Self where F: for<'store> FnOnce(UpdateContext<'store>) -> T<'store> {
        let context = UpdateContext {
            _scope_marker: marker::PhantomData
        };

        Store {
            data: Arc::new(RwLock::new(initializer(context))),
            reductor,
            _event_marker: marker::PhantomData
        }
    }
}

macro_rules! define_store {
    ($store:ident, $root:ident) => {

    }
}
