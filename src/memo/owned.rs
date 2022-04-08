use std::marker;

use crate::memo::{Memo, Refresh};
use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::TypeConstructor;

pub struct OwnedMemo<C, S, T> {
    selector: S,
    store_id: usize,
    last_value: T,
    _marker: marker::PhantomData<*const C>,
}

impl<C, S, T: PartialEq + 'static> OwnedMemo<C, S, T>
where
    C: TypeConstructor,
    S: for<'store> Fn(&C::Type<'store>, ReadContext<'store>) -> T,
{
    pub fn new(store: &Store<C>, selector: S) -> Self {
        let last_value = store.with(|root, cx| selector(root, cx));

        OwnedMemo {
            selector,
            store_id: store.id(),
            last_value,
            _marker: marker::PhantomData,
        }
    }
}

impl<C, S, T: PartialEq + 'static> Memo for OwnedMemo<C, S, T>
where
    C: TypeConstructor + 'static,
    S: for<'store> Fn(&C::Type<'store>, ReadContext<'store>) -> T + 'static,
{
    type RootTC = C;
    type Value<'a, 'b, 'store: 'b> = &'a T;

    fn store_id(&self) -> usize {
        self.store_id
    }

    fn refresh_unchecked<'a, 'b, 'store: 'b>(
        &'a mut self,
        root: &'b C::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<Self::Value<'a, 'b, 'store>> {
        let value = (self.selector)(root, cx);

        let is_changed = value == self.last_value;

        self.last_value = value;

        Refresh {
            is_changed,
            value: &self.last_value,
        }
    }
}
