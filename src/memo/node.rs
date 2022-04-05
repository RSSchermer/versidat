use std::sync::atomic::AtomicU64;
use std::marker;
use std::sync::atomic;

use crate::TypeConstructor;
use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::memo::{Memo, Refresh, Selector};

pub struct NodeMemo<N, C, S> {
    select: S,
    store_id: u64,
    last_version: u64,
    _root_marker: marker::PhantomData<*const C>,
    _node_marker: marker::PhantomData<*const N>,
}

impl<N, C, S> NodeMemo<N, C, S>
    where
        N: TypeConstructor,
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, N::Type<'store>>,
{
    pub fn new(store: &Store<C>, select: S) -> Self {
        let last_version = store.with(|root, cx| select(root, cx).version());

        NodeMemo {
            select,
            store_id: store.id(),
            last_version,
            _root_marker: marker::PhantomData,
            _node_marker: marker::PhantomData,
        }
    }
}

impl<N, C, S> Memo for NodeMemo<N, C, S>
    where
        N: TypeConstructor,
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, N::Type<'store>>
        + Clone,
{
    type RootTC = C;
    type Target<'store> = VersionedCell<'store, N::Type<'store>>;
    type Selector = NodeSelector<N, C, S>;

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
        NodeSelector {
            lens: self.select.clone(),
            _node_marker: marker::PhantomData,
            _store_marker: marker::PhantomData,
        }
    }
}

pub struct NodeSelector<N, C, S> {
    lens: S,
    _node_marker: marker::PhantomData<*const N>,
    _store_marker: marker::PhantomData<*const C>,
}

impl<N, C, S> Selector for NodeSelector<N, C, S>
    where
        N: TypeConstructor,
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, N::Type<'store>>,
{
    type RootTC = C;
    type Target<'store> = VersionedCell<'store, N::Type<'store>>;

    fn select<'a, 'store>(
        &self,
        root: &'a C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> &'a Self::Target<'store> {
        (self.lens)(root, cx)
    }
}
