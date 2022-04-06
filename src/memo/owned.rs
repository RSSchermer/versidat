use std::sync::atomic::AtomicU64;
use std::marker;
use crate::TypeConstructor;
use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::memo::{Memo, Refresh, Selector};
use std::sync::atomic;

pub struct OwnedMemo<C, S, T> {
    select: S,
    store_id: u64,
    last: T,
    _marker: marker::PhantomData<*const C>,
}

impl<C, S, T> OwnedMemo<C, S, T>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> T,
{
    pub fn new(store: &Store<C>, select: S) -> Self {
        let last = store.with(|root, cx| select(root, cx));

        OwnedMemo {
            select,
            store_id: store.id(),
            last,
            _marker: marker::PhantomData,
        }
    }
}

// impl<C, S, T> Memo for OwnedMemo<C, S>
//     where
//         C: TypeConstructor,
//         S: for<'a, 'store> Fn(&'a C::Type<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>
//         + Clone,
// {
//     type RootTC = C;
//     type Target<'store> = VersionedCell<'store, T>;
//     type Selector = CellSelector<C, S>;
//
//     fn refresh<'a, 'store>(
//         &mut self,
//         root: &'a C::Type<'store>,
//         cx: ReadContext<'store>,
//     ) -> Refresh<&'a Self::Target<'store>> {
//         if cx.store_id() != self.store_id {
//             panic!("cannot resolve selector against different store")
//         }
//
//         let cell = (self.select)(root, cx);
//         let version = cell.version();
//         let last_version = self.last_version;
//
//         self.last_version = version;
//
//         if version == last_version {
//             Refresh::Unchanged(cell)
//         } else {
//             Refresh::Changed(cell)
//         }
//     }
//
//     fn selector(&self) -> Self::Selector {
//         CellSelector {
//             lens: self.select.clone(),
//             _marker: marker::PhantomData,
//         }
//     }
// }

pub struct OwnedSelector<C, T> {
    value: T,
    _marker: marker::PhantomData<*const C>
}

impl<C, T> Selector for OwnedSelector<C, T> where C: TypeConstructor, T: Clone {
    type RootTC = C;
    type Target<'a, 'store: 'a> = T;

    fn select<'a, 'store: 'a>(
        &self,
        root: &'a C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> T {
        self.value.clone()
    }
}
