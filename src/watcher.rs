use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use crate::memo::{Memo, ValueResolver};
use crate::store::{OnUpdate, ReadContext, Store};
use crate::TypeConstructor;

pub struct Watcher<C, M>
where
    C: TypeConstructor,
{
    store: Store<C>,
    memo: M,
    on_update: OnUpdate,
}

impl<C, M> Watcher<C, M>
where
    C: TypeConstructor,
    M: Memo<RootTC = C>,
{
    pub fn new(store: &Store<C>, memo: M) -> Self {
        if memo.store_id() != store.id() {
            panic!("memo is not associated with the store passed to the watcher")
        }

        Watcher {
            on_update: store.on_update(),
            memo,
            store: store.clone(),
        }
    }
}

impl<C, M> Stream for Watcher<C, M>
where
    C: TypeConstructor,
    M: Memo<RootTC = C>,
{
    type Item = View<C, M::ValueResolver>;

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
                        resolver: memo.value_resolver(),
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
    resolver: S,
}

impl<C, R> View<C, R>
where
    C: TypeConstructor,
    R: ValueResolver<RootTC = C>,
{
    pub fn with<F, O>(&self, f: F) -> O
    where
        F: for<'a, 'store> FnOnce(R::Value<'a, 'store>, ReadContext<'store>) -> O,
    {
        self.store
            .with(|root, cx| f(self.resolver.select(root, cx), cx))
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
        if memo_0.store_id() != store.id() {
            panic!("memo `0` is not associated with the store passed to this watcher");
        }

        if memo_1.store_id() != store.id() {
            panic!("memo `1` is not associated with the store passed to this watcher");
        }

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
    type Item = View2<C, M0::ValueResolver, M1::ValueResolver>;

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

                    changed |= memo_0.refresh_unchecked(root, cx).is_changed();
                    changed |= memo_1.refresh_unchecked(root, cx).is_changed();

                    changed
                });

                if changed {
                    Poll::Ready(Some(View2 {
                        store: store.clone(),
                        resolver_0: memo_0.value_resolver(),
                        resolver_1: memo_1.value_resolver(),
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

pub struct View2<C, R0, R1>
where
    C: TypeConstructor,
{
    store: Store<C>,
    resolver_0: R0,
    resolver_1: R1,
}

impl<C, R0, R1> View2<C, R0, R1>
where
    C: TypeConstructor,
    R0: ValueResolver<RootTC = C>,
    R1: ValueResolver<RootTC = C>,
{
    pub fn with<F, O>(&self, f: F) -> O
    where
        F: for<'a, 'store> FnOnce((R0::Value<'a, 'store>, R1::Value<'a, 'store>), ReadContext<'store>) -> O,
    {
        self.store.with(|root, cx| {
            let resolver_0 = self.resolver_0.select(root, cx);
            let resolver_1 = self.resolver_1.select(root, cx);

            f((resolver_0, resolver_1), cx)
        })
    }
}
