use std::marker;

use crate::memo::{Memo, MemoLifetime, Refresh};
use crate::store::{ReadContext, Store};
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

impl<'a, 'b, 'store, C, S, T: PartialEq + 'static> MemoLifetime<'a, 'b, 'store>
    for OwnedMemo<C, S, T>
where
    C: TypeConstructor + 'static,
    S: Fn(&'b C::Type<'store>, ReadContext<'store>) -> T + 'static,
{
    type Value = &'a T;
}

impl<C, S, T: PartialEq + 'static> Memo for OwnedMemo<C, S, T>
where
    C: TypeConstructor + 'static,
    S: for<'store> Fn(&C::Type<'store>, ReadContext<'store>) -> T + 'static,
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
        let value = (self.selector)(root, cx);

        let is_changed = value != self.last_value;

        self.last_value = value;

        Refresh {
            is_changed,
            value: &self.last_value,
        }
    }
}
