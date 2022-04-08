use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use crate::memo::Memo;
use crate::store::{OnUpdate, ReadContext, Store};
use crate::TypeConstructor;

// TODO: these don't currently work... The library code compiles, but if you they to create a
// watcher, it fails with an error like this:
//
//     error[E0478]: lifetime bound not satisfied
//       --> examples/playground.rs:53:24
//        |
//     53 |     let mut watcher2 = Watcher2::new(&store, cell_memo, node_memo, |(cell, node), cx| {
//        |                        ^^^^^^^^^^^^^
//
// This does make sense to me, I believe the HRTBs of the watcher function `F` really want to be:
//
//    for<'a, 'b, 'store: 'b> Fn(M::Value<'a, 'b, 'store>, ReadContext<'store>) -> O
//                      ^^^^
//
// However, `for<'a, 'b: 'a>` constraints are currently not supported in HRTBs. It looks like this
// may become possible when Polonius lands: https://github.com/rust-lang/polonius/issues/172
//
// I've also experimented with trying to force the compiler to infer "implied bounds", to no
// success. It's worth pointing out that other places in this codebase also really need
// `for<'a, 'store: 'a>` type HRTB constraints, but in those places it does work because we are
// working with references (e.g. `&'a C::Type<'store>`), and for references the compiler realises
// that `'store: 'a` is implied. It doesn't seem to come to the same realisation if the bounds
// derive from constraints on GATs. I have some faint hope that perhaps in the future implied bounds
// will be automatically inferred for more than just references; that would likely also solve this
// issue.

pub struct Watcher<C, M, F>
where
    C: TypeConstructor,
{
    store: Store<C>,
    f: F,
    on_update: OnUpdate,
    memo: M,
}

impl<C, M, F, O> Watcher<C, M, F>
where
    C: TypeConstructor,
    M: Memo<RootTC = C>,
    F: for<'a, 'b, 'store> Fn(M::Value<'a, 'b, 'store>, ReadContext<'store>) -> O,
{
    pub fn new(store: &Store<C>, memo: M, f: F) -> Self {
        if memo.store_id() != store.id() {
            panic!("memo is not associated with the store passed to the watcher")
        }

        Watcher {
            f,
            store: store.clone(),
            on_update: store.on_update(),
            memo,
        }
    }
}

impl<C, M, F, O> Stream for Watcher<C, M, F>
where
    C: TypeConstructor,
    M: Memo<RootTC = C>,
    F: for<'a, 'b, 'store> Fn(M::Value<'a, 'b, 'store>, ReadContext<'store>) -> O,
{
    type Item = O;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Watcher {
            store,
            f,
            on_update,
            memo,
        } = unsafe { self.get_unchecked_mut() };

        match Pin::new(on_update).poll_next(cx) {
            Poll::Ready(Some(_)) => store.with(|root, cx| {
                let refreshed = memo.refresh_unchecked(root, cx);

                if refreshed.is_changed {
                    Poll::Ready(Some(f(refreshed.value, cx)))
                } else {
                    Poll::Pending
                }
            }),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct Watcher2<C, M0, M1, F>
where
    C: TypeConstructor,
{
    store: Store<C>,
    f: F,
    on_update: OnUpdate,
    memo_0: M0,
    memo_1: M1,
}

impl<C, M0, M1, F, O> Watcher2<C, M0, M1, F>
where
    C: TypeConstructor,
    M0: Memo<RootTC = C>,
    M1: Memo<RootTC = C>,
    F: for<'a, 'b, 'store> Fn(
        (M0::Value<'a, 'b, 'store>, M1::Value<'a, 'b, 'store>),
        ReadContext<'store>,
    ) -> O,
{
    pub fn new(store: &Store<C>, memo_0: M0, memo_1: M1, f: F) -> Self {
        if memo_0.store_id() != store.id() {
            panic!("memo_0 is not associated with the store passed to the watcher")
        }

        if memo_1.store_id() != store.id() {
            panic!("memo_1 is not associated with the store passed to the watcher")
        }

        Watcher2 {
            on_update: store.on_update(),
            f,
            store: store.clone(),
            memo_0,
            memo_1,
        }
    }
}

impl<C, M0, M1, F, O> Stream for Watcher2<C, M0, M1, F>
where
    C: TypeConstructor,
    M0: Memo<RootTC = C>,
    M1: Memo<RootTC = C>,
    F: for<'a, 'b, 'store> Fn(
        (M0::Value<'a, 'b, 'store>, M1::Value<'a, 'b, 'store>),
        ReadContext<'store>,
    ) -> O,
{
    type Item = O;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Watcher2 {
            store,
            f,
            on_update,
            memo_0,
            memo_1,
        } = unsafe { self.get_unchecked_mut() };

        match Pin::new(on_update).poll_next(cx) {
            Poll::Ready(Some(_)) => store.with(|root, cx| {
                let memo_0 = memo_0.refresh_unchecked(root, cx);
                let memo_1 = memo_1.refresh_unchecked(root, cx);

                let mut is_changed = false;

                if memo_0.is_changed {
                    is_changed = true;
                }

                if memo_1.is_changed {
                    is_changed = true;
                }

                if is_changed {
                    Poll::Ready(Some(f((memo_0.value, memo_1.value), cx)))
                } else {
                    Poll::Pending
                }
            }),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
