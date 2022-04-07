use std::sync::atomic::AtomicU64;
use std::marker;
use crate::TypeConstructor;
use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::memo::{Memo, Refresh, ValueResolver};
use std::sync::atomic;

pub struct OptionCellMemo<C, S> {
    select: S,
    store_id: usize,
    last_version: Option<u64>,
    _marker: marker::PhantomData<*const C>,
}

impl<C, S, T: 'static> OptionCellMemo<C, S>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> Option<&'a VersionedCell<'store, T>>,
{
    pub fn new(store: &Store<C>, select: S) -> Self {
        let last_version = store.with(|root, cx| select(root, cx).map(|c| c.version()));

        OptionCellMemo {
            select,
            store_id: store.id(),
            last_version,
            _marker: marker::PhantomData,
        }
    }
}

impl<C, S, T: 'static> Memo for OptionCellMemo<C, S>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> Option<&'a VersionedCell<'store, T>>
        + Clone,
{
    type RootTC = C;
    type Value<'a, 'store: 'a> = Option<&'a VersionedCell<'store, T>>;
    type ValueResolver = OptionCellResolver<C, S>;

    fn store_id(&self) -> usize {
        self.store_id
    }

    fn refresh_unchecked<'a, 'store>(
        &mut self,
        root: &'a C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<Self::Value<'a, 'store>> {
        let cell = (self.select)(root, cx);
        let version = cell.map(|c| c.version());
        let last_version = self.last_version;

        self.last_version = version;

        if version == last_version {
            Refresh::Unchanged(cell)
        } else {
            Refresh::Changed(cell)
        }
    }

    fn value_resolver(&self) -> Self::ValueResolver {
        OptionCellResolver {
            lens: self.select.clone(),
            _marker: marker::PhantomData,
        }
    }
}

pub struct OptionCellResolver<C, S> {
    lens: S,
    _marker: marker::PhantomData<*const C>,
}

impl<C, S, T: 'static> ValueResolver for OptionCellResolver<C, S>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> Option<&'a VersionedCell<'store, T>>,
{
    type RootTC = C;
    type Value<'a, 'store: 'a> = Option<&'a VersionedCell<'store, T>>;

    fn resolve<'a, 'store: 'a>(
        &self,
        root: &'a C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Self::Value<'a, 'store> {
        (self.lens)(root, cx)
    }
}
