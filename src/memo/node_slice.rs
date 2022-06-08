use std::hash::{Hash, Hasher};
use std::marker;

use seahash::SeaHasher;

use crate::memo::{Memo, MemoLifetime, Refresh};
use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::TypeConstructor;

pub struct NodeSliceMemo<N, C, S> {
    selector: S,
    store_id: usize,
    last_version: u64,
    _marker: marker::PhantomData<(*const C, *const N)>,
}

impl<N, C, S> NodeSliceMemo<N, C, S>
where
    N: TypeConstructor,
    C: TypeConstructor,
    S: for<'a, 'store> Fn(
        &'a C::Type<'store>,
        ReadContext<'store>,
    ) -> &'a [VersionedCell<'store, N::Type<'store>>],
{
    pub fn new(store: &Store<C>, selector: S) -> Self {
        let last_version = store.with(|root, cx| {
            let mut hasher = SeaHasher::new();

            for node in selector(root, cx) {
                node.version().hash(&mut hasher);
            }

            hasher.finish()
        });

        NodeSliceMemo {
            selector,
            store_id: store.id(),
            last_version,
            _marker: marker::PhantomData,
        }
    }
}

impl<'a, 'b, 'store, N, C, S> MemoLifetime<'a, 'b, 'store> for NodeSliceMemo<N, C, S>
where
    N: TypeConstructor + 'static,
    C: TypeConstructor + 'static,
    S: Fn(&'b C::Type<'store>, ReadContext<'store>) -> &'b [VersionedCell<'store, N::Type<'store>>]
        + 'static,
{
    type Value = &'b [VersionedCell<'store, N::Type<'store>>];
}

impl<N, C, S> Memo for NodeSliceMemo<N, C, S>
where
    N: TypeConstructor + 'static,
    C: TypeConstructor + 'static,
    S: for<'a, 'store> Fn(
            &'a C::Type<'store>,
            ReadContext<'store>,
        ) -> &'a [VersionedCell<'store, N::Type<'store>>]
        + 'static,
{
    type RootTC = C;

    fn store_id(&self) -> usize {
        self.store_id
    }

    fn refresh_unchecked<'a, 'b, 'store: 'b>(
        &'a mut self,
        root: &'b C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<<Self as MemoLifetime<'a, 'b, 'store>>::Value> {
        let slice = (self.selector)(root, cx);
        let mut hasher = SeaHasher::new();

        for node in slice {
            node.version().hash(&mut hasher);
        }

        let version = hasher.finish();
        let last_version = self.last_version;

        self.last_version = version;

        Refresh {
            value: slice,
            is_changed: version != last_version,
        }
    }
}
