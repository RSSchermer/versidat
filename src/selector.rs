use std::marker;
use crate::{VersionedCell, Store};
use futures::Stream;
use std::ops::Deref;
use crate::on_version_change::OnVersionChange;
use std::sync::{Arc, RwLock};

pub trait ViewModel<T>: Borrow<T> {
    type OnChange: Stream;

    fn on_change(&self) -> Self::OnChange;
}

impl<T> ViewModel<T> for VersionedCell<T> {
    type OnChange = OnVersionChange;

    fn get<'a>(&self) -> &T {
        self.deref()
    }

    fn on_change(&self) -> Self::OnChange {
        VersionedCell::on_version_change(self)
    }
}

pub struct Selector<F, T, P> {
    data: T,
    f: F,
    on_version_change: OnVersionChange,
    _selected_marker: marker::PhantomData<*const P>,
}

impl<F, T, P> Selector<F, T, P> where F: Fn(&T) -> &VersionedCell<P> {
    pub fn new(data: T, f: F) -> Self {
        let on_version_change = f(&data).on_version_change();

        Selector {
            data,
            f,
            on_version_change,
            _selected_marker: marker::PhantomData
        }
    }
}

impl<F, T, P> ViewModel<P> for Selector<F, T, P> where F: Fn(&T) -> &VersionedCell<P> {
    type OnChange = OnVersionChange;

    fn get(&self) -> &P {
        (self.f)(&self.data)
    }

    fn on_change(&self) -> Self::OnChange {
        self.on_version_change.clone()
    }
}

pub struct Aggregator<T, P> {
    aggregate: T,
    _marker: marker::PhantomData<*const P>
}

impl<T, P> Aggregator<T, P> where T: IntoIterator, T::Item: ViewModel<P> {

}
