use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

use crate::memo::{Memo, MemoLifetime};
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
    initial: bool,
}

impl<C, M, F, O> Watcher<C, M, F>
where
    C: TypeConstructor,
    M: Memo<RootTC = C>,
    F: for<'a, 'b, 'store> Fn(
        <M as MemoLifetime<'a, 'b, 'store>>::Value,
        ReadContext<'store>,
    ) -> Option<O>,
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
            initial: true,
        }
    }
}

impl<C, M, F, O> Stream for Watcher<C, M, F>
where
    C: TypeConstructor,
    M: Memo<RootTC = C>,
    F: for<'a, 'b, 'store> Fn(
        <M as MemoLifetime<'a, 'b, 'store>>::Value,
        ReadContext<'store>,
    ) -> Option<O>,
{
    type Item = O;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Watcher {
            store,
            f,
            on_update,
            memo,
            initial,
        } = unsafe { self.get_unchecked_mut() };

        if *initial {
            *initial = false;

            return store.with(|root, cx| {
                let refreshed = memo.refresh_unchecked(root, cx);

                Poll::Ready(f(refreshed.value, cx))
            });
        }

        match Pin::new(on_update).poll_next(cx) {
            Poll::Ready(Some(_)) => store.with(|root, cx| {
                let refreshed = memo.refresh_unchecked(root, cx);

                if refreshed.is_changed {
                    Poll::Ready(f(refreshed.value, cx))
                } else {
                    Poll::Pending
                }
            }),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

macro_rules! watcher {
    ($watcher:ident, $($memo:ident $name:literal),*) => {
        #[allow(non_snake_case)]
        pub struct $watcher<C, $($memo,)* F>
        where
            C: TypeConstructor,
        {
            store: Store<C>,
            f: F,
            on_update: OnUpdate,
            $($memo: $memo,)*
            initial: bool
        }

        #[allow(non_snake_case)]
        impl<C, $($memo,)* F, O> $watcher<C, $($memo,)* F>
        where
            C: TypeConstructor,
            $($memo: Memo<RootTC = C>,)*
            F: for<'a, 'b, 'store> Fn(
                (
                    $(<$memo as MemoLifetime<'a, 'b, 'store>>::Value,)*
                ),
                ReadContext<'store>,
            ) -> Option<O>,
        {
            pub fn new(store: &Store<C>, $($memo: $memo,)* f: F) -> Self {
                $(
                    if $memo.store_id() != store.id() {
                        panic!("{} is not associated with the store passed to the watcher", $name)
                    }
                )*

                $watcher {
                    on_update: store.on_update(),
                    f,
                    store: store.clone(),
                    $($memo,)*
                    initial: true
                }
            }
        }

        #[allow(non_snake_case)]
        impl<C, $($memo,)* F, O> Stream for $watcher<C, $($memo,)* F>
        where
            C: TypeConstructor,
            $($memo: Memo<RootTC = C>,)*
            F: for<'a, 'b, 'store> Fn(
                (
                    $(<$memo as MemoLifetime<'a, 'b, 'store>>::Value,)*
                ),
                ReadContext<'store>,
            ) -> Option<O>,
        {
            type Item = O;

            fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                let $watcher {
                    store,
                    f,
                    on_update,
                    $($memo,)*
                    initial
                } = unsafe { self.get_unchecked_mut() };

                if *initial {
                    *initial = false;

                    return store.with(|root, cx| {
                        $(let $memo = $memo.refresh_unchecked(root, cx);)*

                        Poll::Ready(f(($($memo.value),*), cx))
                    });
                }

                match Pin::new(on_update).poll_next(cx) {
                    Poll::Ready(Some(_)) => store.with(|root, cx| {
                        $(let $memo = $memo.refresh_unchecked(root, cx);)*

                        let mut is_changed = false;

                        $(
                            if $memo.is_changed {
                                is_changed = true;
                            }
                        )*

                        if is_changed {
                            Poll::Ready(f(($($memo.value),*), cx))
                        } else {
                            Poll::Pending
                        }
                    }),
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }
}

watcher!(Watcher2, M0 "memo `0`", M1 "memo `1`");
watcher!(Watcher3, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`");
watcher!(Watcher4, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`");
watcher!(Watcher5, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`");
watcher!(Watcher6, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`");
watcher!(Watcher7, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`");
watcher!(Watcher8, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`");
watcher!(Watcher9, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`", M8 "memo `8`");
watcher!(Watcher10, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`", M8 "memo `8`", M9 "memo `9`");
watcher!(Watcher11, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`", M8 "memo `8`", M9 "memo `9`", M10 "memo `10`");
watcher!(Watcher12, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`", M8 "memo `8`", M9 "memo `9`", M10 "memo `10`", M11 "memo `11`");
watcher!(Watcher13, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`", M8 "memo `8`", M9 "memo `9`", M10 "memo `10`", M11 "memo `11`", M12 "memo `12`");
watcher!(Watcher14, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`", M8 "memo `8`", M9 "memo `9`", M10 "memo `10`", M11 "memo `11`", M12 "memo `12`", M13 "memo `13`");
watcher!(Watcher15, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`", M8 "memo `8`", M9 "memo `9`", M10 "memo `10`", M11 "memo `11`", M12 "memo `12`", M13 "memo `13`", M14 "memo `14`");
watcher!(Watcher16, M0 "memo `0`", M1 "memo `1`", M2 "memo `2`", M3 "memo `3`", M4 "memo `4`", M5 "memo `5`", M6 "memo `6`", M7 "memo `7`", M8 "memo `8`", M9 "memo `9`", M10 "memo `10`", M11 "memo `11`", M12 "memo `12`", M13 "memo `13`", M14 "memo `14`", M15 "memo `15`");
