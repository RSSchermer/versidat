use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use crate::TypeConstructor;
use crate::store::{Store, OnUpdate, ReadContext};
use crate::memo::{Memo, Selector};

pub struct Watcher<C, M>
    where
        C: TypeConstructor,
{
    store: Store<C>,
    memo: M,
    on_update: OnUpdate,
}

impl<C, M> Stream for Watcher<C, M>
    where
        C: TypeConstructor,
        M: Memo<RootTC = C>,
{
    type Item = View<C, M::Selector>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Watcher {
            store,
            memo,
            on_update,
        } = unsafe { self.get_unchecked_mut() };

        match Pin::new(on_update).poll_next(cx) {
            Poll::Ready(Some(_)) => {
                let changed = store.with(|root, cx| memo.refresh(root, cx).is_changed());

                if changed {
                    Poll::Ready(Some(View {
                        store: store.clone(),
                        selector: memo.selector(),
                    }))
                } else {
                    Poll::Pending
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct View<C, S>
    where
        C: TypeConstructor,
{
    store: Store<C>,
    selector: S,
}

impl<C, S> View<C, S>
    where
        C: TypeConstructor,
        S: Selector<RootTC = C>,
{
    pub fn with<F, R>(&self, f: F) -> R
        where
            F: for<'a, 'store> FnOnce(S::Target<'a, 'store>, ReadContext<'store>) -> R,
    {
        self.store.with(|root, cx| f(self.selector.select(root, cx), cx))
    }
}

pub struct Watcher2<C, M0, M1>
    where
        C: TypeConstructor,
{
    store: Store<C>,
    memo_0: M0,
    memo_1: M1,
    on_update: OnUpdate,
}

impl<C, M0, M1> Watcher2<C, M0, M1>
    where
        C: TypeConstructor,
        M0: Memo<RootTC = C>,
        M1: Memo<RootTC = C>,
{
    pub fn new(store: &Store<C>, memo_0: M0, memo_1: M1) -> Self {
        Watcher2 {
            on_update: store.on_update(),
            store: store.clone(),
            memo_0,
            memo_1,

        }
    }
}

impl<C, M0, M1> Stream for Watcher2<C, M0, M1>
    where
        C: TypeConstructor,
        M0: Memo<RootTC = C>,
        M1: Memo<RootTC = C>,
{
    type Item = View2<C, M0::Selector, M1::Selector>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Watcher2 {
            store,
            memo_0,
            memo_1,
            on_update,
        } = unsafe { self.get_unchecked_mut() };

        match Pin::new(on_update).poll_next(cx) {
            Poll::Ready(Some(_)) => {
                let changed = store.with(|root, cx| {
                    let mut changed = false;

                    changed |= memo_0.refresh(root, cx).is_changed();
                    changed |= memo_1.refresh(root, cx).is_changed();

                    changed
                });

                if changed {
                    Poll::Ready(Some(View2 {
                        store: store.clone(),
                        selector_0: memo_0.selector(),
                        selector_1: memo_1.selector(),
                    }))
                } else {
                    Poll::Pending
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct View2<C, S0, S1>
    where
        C: TypeConstructor,
{
    store: Store<C>,
    selector_0: S0,
    selector_1: S1,
}

impl<C, M0, M1> View2<C, M0, M1>
    where
        C: TypeConstructor,
        M0: Selector<RootTC = C>,
        M1: Selector<RootTC = C>,
{
    pub fn with<F, R>(&self, f: F) -> R
        where
            F: for<'a, 'store> FnOnce((M0::Target<'a, 'store>, M1::Target<'a, 'store>), ReadContext<'store>) -> R,
    {
        self.store.with(|root, cx| {
            let selector_0 = self.selector_0.select(root, cx);
            let selector_1 = self.selector_1.select(root, cx);

            f((selector_0, selector_1), cx)
        })
    }
}
