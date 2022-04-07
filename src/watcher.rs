use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use crate::memo::Memo;
use crate::store::{OnUpdate, ReadContext, Store};
use crate::TypeConstructor;

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
    F: for<'a, 'b, 'store> Fn(M::Value<'a, 'b, 'store>, ReadContext<'store>) -> O
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
    F: for<'a, 'b, 'store> Fn(M::Value<'a, 'b, 'store>, ReadContext<'store>) -> O
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
            Poll::Ready(Some(_)) => {
                store.with(|root, cx| {
                    let refreshed = memo.refresh_unchecked(root, cx);

                    if refreshed.is_changed {
                        Poll::Ready(Some(f(refreshed.value, cx)))
                    } else {
                        Poll::Pending
                    }
                })
            }
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
        F: for<'a, 'b, 'store> Fn((M0::Value<'a, 'b, 'store>, M1::Value<'a, 'b, 'store>), ReadContext<'store>) -> O
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
        F: for<'a, 'b, 'store> Fn((M0::Value<'a, 'b, 'store>, M1::Value<'a, 'b, 'store>), ReadContext<'store>) -> O
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
            Poll::Ready(Some(_)) => {
                store.with(|root, cx| {
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
                })
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
