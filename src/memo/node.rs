use std::marker;

use crate::memo::{Memo, Refresh};
use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::TypeConstructor;

pub struct NodeMemo<N, C, S> {
    selector: S,
    store_id: usize,
    last_version: u64,
    _root_marker: marker::PhantomData<*const C>,
    _node_marker: marker::PhantomData<*const N>,
}

impl<N, C, S> NodeMemo<N, C, S>
where
    N: TypeConstructor,
    C: TypeConstructor,
    S: for<'a, 'store> Fn(
        &'a C::Type<'store>,
        ReadContext<'store>,
    ) -> &'a VersionedCell<'store, N::Type<'store>>,
{
    pub fn new(store: &Store<C>, selector: S) -> Self {
        let last_version = store.with(|root, cx| selector(root, cx).version());

        NodeMemo {
            selector,
            store_id: store.id(),
            last_version,
            _root_marker: marker::PhantomData,
            _node_marker: marker::PhantomData,
        }
    }
}

impl<N, C, S> Memo for NodeMemo<N, C, S>
where
    N: TypeConstructor + 'static,
    C: TypeConstructor + 'static,
    S: for<'a, 'store> Fn(
            &'a C::Type<'store>,
            ReadContext<'store>,
        ) -> &'a VersionedCell<'store, N::Type<'store>>
        + 'static,
{
    type RootTC = C;
    type Value<'a, 'b, 'store: 'b> = &'b VersionedCell<'store, N::Type<'store>>;

    fn store_id(&self) -> usize {
        self.store_id
    }

    fn refresh_unchecked<'a, 'b, 'store>(
        &'a mut self,
        root: &'b C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<Self::Value<'a, 'b, 'store>> {
        let cell = (self.selector)(root, cx);
        let version = cell.version();
        let last_version = self.last_version;

        self.last_version = version;

        Refresh {
            value: cell,
            is_changed: version == last_version
        }
    }
}
