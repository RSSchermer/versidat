use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::marker;

use seahash::SeaHasher;

use crate::store::{ReadContext, Store};
use crate::versioned_cell::VersionedCell;
use crate::TypeConstructor;

// TODO: this doesn't work... Below is my best attempt. It will actually compile here, but when
// one tries to create a new `CellIterMemo`, it fails with an error like this
//
//     error: implementation of `IntoIterSelector` is not general enough
//       --> examples/playground.rs:51:25
//        |
//     51 |     let mut iter_memo = CellIterMemo::new(&store, |root: &MyRoot, cx| root.elements.iter());
//        |                         ^^^^^^^^^^^^^^^^^ implementation of `IntoIterSelector` is not general enough
//        |
//        = note: `[closure@examples/playground.rs:51:51: 51:91]` must implement `IntoIterSelector<'0, 'store, MyRootTC, Element>`, for any lifetime `'0`...
//        = note: ...but it actually implements `IntoIterSelector<'_, '_, MyRootTC, Element>`
//
// Seemingly, the compiler uses the blanket implementation below to generate one specific
// implementation. A later stage (borrow check?) then finds that the bound must hold for any
// `<'a, 'store>`, but only finds the one specific implementation. The compiler seems (currently)
// not quite smart enough to realise that the blanket implementation could be used to generate an
// implementation for every `<'a, 'store>`.
//
// I'm not sure if there is a different way to express this that does work on current nightly, or
// if the compiler can/will ever be upgraded to accept this code. Note that there is also the
// definite possibility that the compiler is correctly rejecting this code and that it is only my
// understanding that is lacking.

pub trait IntoIterSelector<'a, 'store, C: TypeConstructor, T: 'static> {
    type IntoIter: IntoIterator<Item = Self::Item>;

    type Item: Borrow<VersionedCell<'store, T>> + 'a;

    fn select(&self, root: &'a C::Type<'store>, cx: ReadContext<'store>) -> Self::IntoIter;
}

impl<'a, 'store: 'a, C: TypeConstructor, T: 'static, F, I> IntoIterSelector<'a, 'store, C, T> for F
where
    F: Fn(&'a C::Type<'store>, ReadContext<'store>) -> I,
    I: IntoIterator,
    I::Item: Borrow<VersionedCell<'store, T>> + 'a,
{
    type IntoIter = I;
    type Item = I::Item;

    fn select(
        &self,
        root: &'a <C as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Self::IntoIter {
        self(root, cx)
    }
}

pub struct CellIterMemo<C, S, T> {
    selector: S,
    store_id: usize,
    last_version: u64,
    _marker: marker::PhantomData<(*const C, *const T)>,
}

impl<C, S, T: 'static> CellIterMemo<C, S, T>
where
    C: TypeConstructor,
    S: for<'a, 'store> IntoIterSelector<'a, 'store, C, T>,
{
    pub fn new(store: &Store<C>, selector: S) -> Self {
        let last_version = store.with(|root, cx| {
            let mut hasher = SeaHasher::new();

            for cell in selector.select(root, cx) {
                cell.borrow().version().hash(&mut hasher);
            }

            hasher.finish()
        });

        CellIterMemo {
            selector,
            store_id: store.id(),
            last_version,
            _marker: marker::PhantomData,
        }
    }
}
