use crate::store::ReadContext;
use crate::TypeConstructor;

pub struct Refresh<T> {
    pub value: T,
    pub is_changed: bool,
}

mod sealed {
    pub trait Sealed: Sized {}
    pub struct Bounds<T>(T);
    impl<T> Sealed for Bounds<T> {}
}
use sealed::{Bounds, Sealed};

pub trait MemoLifetime<'a, 'b, 'store, ImplicitBounds: Sealed = Bounds<&'b &'store ()>> {
    type Value;
}

pub trait Memo: for<'a, 'b, 'store> MemoLifetime<'a, 'b, 'store> {
    type RootTC: TypeConstructor;

    fn store_id(&self) -> usize;

    fn refresh_unchecked<'a, 'b, 'store: 'b>(
        &'a mut self,
        root: &'b <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<<Self as MemoLifetime<'a, 'b, 'store>>::Value>;

    fn refresh<'a, 'b, 'store: 'b>(
        &'a mut self,
        root: &'b <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<<Self as MemoLifetime<'a, 'b, 'store>>::Value> {
        if self.store_id() != cx.store_id() {
            panic!(
                "memo is associated with a different store than the read context that was passed"
            );
        }

        self.refresh_unchecked(root, cx)
    }
}
