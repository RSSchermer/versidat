use std::cell::Cell;
use std::marker;

#[derive(Clone, Copy)]
pub struct ReadContext<'store> {
    _scope_marker: marker::PhantomData<Cell<&'store ()>>,
}

impl<'store> ReadContext<'store> {
    #[doc(hidden)]
    pub unsafe fn new() -> ReadContext<'store> {
        ReadContext {
            _scope_marker: marker::PhantomData,
        }
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
pub struct UpdateContextProvider {
    next_version: u64,
}

impl UpdateContextProvider {
    #[doc(hidden)]
    pub fn new() -> Self {
        UpdateContextProvider { next_version: 0 }
    }

    #[doc(hidden)]
    pub unsafe fn update_context<'store>(&mut self) -> UpdateContext<'store> {
        UpdateContext {
            next_version: &mut self.next_version as *mut u64,
            _scope_marker: marker::PhantomData,
        }
    }
}
