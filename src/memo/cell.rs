use std::marker;

use crate::memo::{Memo, Refresh, ValueResolver};
use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::TypeConstructor;

pub struct CellMemo<C, S> {
    select: S,
    store_id: usize,
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
    type Value<'store> = VersionedCell<'store, T>;
    type ValueResolver = CellResolver<C, S>;

    fn store_id(&self) -> usize {
        self.store_id
    }

    fn refresh_unchecked<'a, 'store>(
        &mut self,
        root: &'a C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<&'a Self::Value<'store>> {
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

    fn value_resolver(&self) -> Self::ValueResolver {
        CellResolver {
            lens: self.select.clone(),
            _marker: marker::PhantomData,
        }
    }
}

pub struct CellResolver<C, S> {
    lens: S,
    _marker: marker::PhantomData<*const C>,
}

impl<C, S, T> ValueResolver for CellResolver<C, S>
where
    C: TypeConstructor,
    S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>,
{
    type RootTC = C;
    type Value<'store> = VersionedCell<'store, T>;

    fn select<'a, 'store>(
        &self,
        root: &'a C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> &'a Self::Value<'store> {
        (self.lens)(root, cx)
    }
}
