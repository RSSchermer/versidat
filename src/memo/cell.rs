use std::sync::atomic::AtomicU64;
use std::marker;
use crate::TypeConstructor;
use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::memo::{Memo, Refresh, Selector};
use std::sync::atomic;

pub struct CellMemo<C, S> {
    select: S,
    store_id: u64,
    last_version: u64,
    _marker: marker::PhantomData<*const C>,
}

impl<C, S, T> CellMemo<C, S>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>,
{
    pub fn new(store: &Store<C>, select: S) -> Self {
        let last_version = store.with(|root, cx| select(root, cx).version());

        CellMemo {
            select,
            store_id: store.id(),
            last_version,
            _marker: marker::PhantomData,
        }
    }
}

impl<C, S, T> Memo for CellMemo<C, S>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>
        + Clone,
{
    type RootTC = C;
    type Target<'store> = VersionedCell<'store, T>;
    type Selector = CellSelector<C, S>;

    fn refresh<'a, 'store>(
        &mut self,
        root: &'a C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<&'a Self::Target<'store>> {
        if cx.store_id() != self.store_id {
            panic!("cannot resolve selector against different store")
        }

        let cell = (self.select)(root, cx);
        let version = cell.version();
        let last_version = self.last_version;

        self.last_version = version;

        if version == last_version {
            Refresh::Unchanged(cell)
        } else {
            Refresh::Changed(cell)
        }
    }

    fn selector(&self) -> Self::Selector {
        CellSelector {
            lens: self.select.clone(),
            _marker: marker::PhantomData,
        }
    }
}

pub struct CellSelector<C, S> {
    lens: S,
    _marker: marker::PhantomData<*const C>,
}

impl<C, S, T> Selector for CellSelector<C, S>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>,
{
    type RootTC = C;
    type Target<'store> = VersionedCell<'store, T>;

    fn select<'a, 'store>(
        &self,
        root: &'a C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> &'a Self::Target<'store> {
        (self.lens)(root, cx)
    }
}
