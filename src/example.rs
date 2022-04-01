use crate::on_update::{OnUpdate, UpdateBroadcaster};
use crate::{ReadContext, UpdateContext, UpdateContextProvider, VersionedCell};
use futures::{Stream, StreamExt};
use seahash::SeaHasher;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::marker;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::{atomic, Arc, RwLock};
use std::task::{Context, Poll};

pub struct Root<'store> {
    element: VersionedCell<'store, LeafElement>,
    element2: VersionedCell<'store, NodeElement<'store>>,
    leaf_elements: Vec<VersionedCell<'store, LeafElement>>,
    node_elements: Vec<VersionedCell<'store, NodeElement<'store>>>,
}

pub struct NodeElement<'store> {
    a: u32,
    element: Option<VersionedCell<'store, LeafElement>>,
}

pub struct LeafElement {
    a: u32,
    b: String,
}

struct Data {
    root: Root<'static>,
    update_context_provider: UpdateContextProvider,
}

struct Shared {
    data: RwLock<Data>,
}

impl Shared {
    fn with<F, R>(&self, f: F) -> R
    where
        F: for<'store> FnOnce(&'store Root<'store>, ReadContext<'store>) -> R,
    {
        let lock = self.data.read().expect("poisoned");

        unsafe {
            f(
                ::std::mem::transmute::<&Root<'static>, _>(&lock.root),
                ReadContext::new(),
            )
        }
    }
}

pub struct Store {
    shared: Arc<Shared>,
    update_broadcaster: UpdateBroadcaster,
}

impl Store {
    pub fn initialize<F>(initializer: F) -> Self
    where
        F: for<'store> FnOnce(UpdateContext<'store>) -> Root<'store>,
    {
        let mut update_context_provider = UpdateContextProvider::new();

        let root: Root<'static> =
            unsafe { std::mem::transmute(initializer(update_context_provider.update_context())) };

        let data = Data {
            root,
            update_context_provider,
        };

        Store {
            shared: Arc::new(Shared {
                data: RwLock::new(data),
            }),
            update_broadcaster: UpdateBroadcaster::new(),
        }
    }

    pub fn with<F, R>(&self, f: F) -> R
    where
        F: for<'store> FnOnce(&'store Root<'store>, ReadContext<'store>) -> R,
    {
        self.shared.with(f)
    }

    pub fn update<F>(&self, f: F)
    where
        F: for<'store> FnOnce(&mut Root<'store>, UpdateContext<'store>),
    {
        let mut lock = self.shared.data.write().expect("poisoned");

        let Data {
            root,
            update_context_provider,
        } = &mut *lock;

        let result = unsafe {
            f(
                ::std::mem::transmute::<&mut Root<'static>, _>(root),
                update_context_provider.update_context(),
            );
        };

        self.update_broadcaster.broadcast();

        result
    }

    pub fn on_update(&self) -> OnUpdate {
        self.update_broadcaster.listener()
    }
}

pub struct LeafView<S> {
    select: S,
    shared: Arc<Shared>,
    last_version: u64,
    on_update: OnUpdate,
}

impl<S, T> LeafView<S>
where
    S: for<'a, 'store> Fn(&'a Root<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>,
{
    pub fn new(store: &Store, select: S) -> Self {
        let last_version = store.with(|root, cx| select(root, cx).version());

        LeafView {
            select,
            shared: store.shared.clone(),
            last_version,
            on_update: store.on_update(),
        }
    }
}

pub struct LeafSelector<S> {
    select: S,
    shared: Arc<Shared>,
}

impl<S, T> LeafSelector<S>
where
    S: for<'a, 'store> Fn(&'a Root<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>,
{
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.shared
            .with(|root, cx| f((self.select)(root, cx).deref(cx)))
    }
}

// This doesn't work unfortunately... At use, it seems to do the `impl` for `F` below, only for one
// specific pair of lifetimes. But when it does the super-trait bounds check on `NodeSelectFn` it
// realizes that it must hold for *any* pair of lifetimes and reports an error. It seems to me that
// the blanket implementation would in theory provide an implementation for *any* pair of lifetimes,
// but it seems to compiler is not quite smart enough to understand this.
//
// pub trait NodeElementSelectFn:
//     for<'a, 'store> Fn(
//     &'a Root<'store>,
//     ReadContext<'store>,
// ) -> &'a VersionedCell<'store, NodeElement<'store>>
// {
// }
//
// impl<F> NodeElementSelectFn for F where
//     F: for<'a, 'store> Fn(
//         &'a Root<'store>,
//         ReadContext<'store>,
//     ) -> &'a VersionedCell<'store, NodeElement<'store>>
// {
// }

pub struct NodeElementView<S> {
    select: S,
    shared: Arc<Shared>,
    last_version: u64,
    on_update: OnUpdate,
}

impl<S> NodeElementView<S>
where
    S: for<'a, 'store> Fn(
        &'a Root<'store>,
        ReadContext<'store>,
    ) -> &'a VersionedCell<'store, NodeElement<'store>>,
{
    pub fn new(store: &Store, select: S) -> Self {
        let last_version = store.with(|root, cx| select(root, cx).version());

        NodeElementView {
            select,
            shared: store.shared.clone(),
            last_version,
            on_update: store.on_update(),
        }
    }
}

pub struct LeaveSliceView<S> {
    select: S,
    shared: Arc<Shared>,
    last_version: u64,
    on_update: OnUpdate,
}

impl<S, T> LeaveSliceView<S>
where
    S: for<'a, 'store> Fn(&'a Root<'store>, ReadContext<'store>) -> &'a [VersionedCell<'store, T>],
{
    pub fn new(store: &Store, select: S) -> Self {
        let last_version = store.with(|root, cx| {
            let mut hasher = SeaHasher::new();

            for element in select(root, cx).into_iter() {
                element.version().hash(&mut hasher)
            }

            hasher.finish()
        });

        LeaveSliceView {
            shared: store.shared.clone(),
            select,
            last_version,
            on_update: store.on_update(),
        }
    }
}

pub struct OptionLeaveSliceView<S> {
    select: S,
    shared: Arc<Shared>,
    last_version: Option<u64>,
    on_update: OnUpdate,
}

impl<S, T> OptionLeaveSliceView<S>
where
    S: for<'a, 'store> Fn(
        &'a Root<'store>,
        ReadContext<'store>,
    ) -> Option<&'a [VersionedCell<'store, T>]>,
{
    pub fn new(store: &Store, select: S) -> Self {
        let last_version = store.with(|root, cx| {
            select(root, cx).map(|slice| {
                let mut hasher = SeaHasher::new();

                for element in slice.into_iter() {
                    element.version().hash(&mut hasher)
                }

                hasher.finish()
            })
        });

        OptionLeaveSliceView {
            shared: store.shared.clone(),
            select,
            last_version,
            on_update: store.on_update(),
        }
    }
}

macro_rules! gen_node_slice_view {
    ($node:ident, $view:ident) => {
        pub struct $view<S> {
            select: S,
        }

        impl<S> $view<S>
        where
            S: for<'a, 'store> Fn(
                &'a Root<'store>,
                ReadContext<'store>,
            ) -> &'a [VersionedCell<'store, $node<'store>>],
        {
            pub fn new(store: &Store, select: S) -> Self {
                todo!();
            }
        }
    };
}

gen_node_slice_view!(NodeElement, NodeElementSliceView);

// pub trait IterSelectFn<T>: for<'a, 'store> Fn(&'a Root<'store>, ReadContext<'store>) -> Self::IntoIter<'a, 'store> {
//     type IntoIter<'a, 'store>: IntoIterator<Item=&'a VersionedCell<'store, T>> where 'store: 'a, T: 'a;
// }
//
// // impl<T, F, I> IterSelectFn<T> for F where F: for<'a, 'store> Fn(&'a Root<'store>, ReadContext<'store>) -> I {
// //     type IntoIter = I;
// // }
//
// pub struct IterLeafView<S, T> {
//     select: S,
//     _marker: marker::PhantomData<*const T>
// }
//
// impl<S, T, I> IterLeafView<S, T> where for<'a, 'store>
//     S: Fn(&'a Root<'store>, ReadContext<'store>) -> I,
//     I: IntoIterator,
//     I::Item: std::borrow::Borrow<VersionedCell<'store, T>>
// {
//     pub fn new(store: &Store, select: S) -> Self {
//         todo!();
//     }
// }

// pub struct OnSelector<S, T> {
//     select: S,
//     shared: Arc<Shared>,
//     last_version: u64,
//     on_update: OnUpdate,
//     _marker: marker::PhantomData<T>
// }
//
// impl<'store, S, T> OnSelector<S, T>
// where
//     S: Fn(&'store Root<'store>, ReadContext<'store>) -> &'store VersionedCell<'store, T>,
//     T: 'store
// {
//     fn new(store: &Store, f: S) -> Self {
//         let last_version = store.with(|root, cx| f(root, cx).version());
//
//         OnSelector {
//             shared: store.shared.clone(),
//             select: f,
//             last_version,
//             on_update: store.on_update(),
//             _marker: marker::PhantomData
//         }
//     }
// }
//
// impl<S, T> Stream for OnSelector<S>
// where
//     S: for<'a, 'store> Fn(&'a Root<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>
//         + Clone,
// {
//     type Item = Selector<S>;
//
//     fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         let OnSelector {
//             select,
//             shared,
//             last_version,
//             on_update,
//         } = unsafe { self.get_unchecked_mut() };
//
//         match Pin::new(on_update).poll_next(cx) {
//             Poll::Ready(Some(_)) => {
//                 let version = shared.with(|root, cx| select(root, cx).version());
//
//                 if version != *last_version {
//                     *last_version = version;
//
//                     Poll::Ready(Some(Selector {
//                         select: select.clone(),
//                         shared: shared.clone(),
//                     }))
//                 } else {
//                     Poll::Pending
//                 }
//             }
//             Poll::Ready(None) => Poll::Ready(None),
//             Poll::Pending => Poll::Pending,
//         }
//     }
// }
//
// pub struct Selector<S> {
//     select: S,
//     shared: Arc<Shared>,
// }
//
// impl<S, T> Selector<S>
// where
//     S: for<'a, 'store> Fn(&'a Root<'store>, ReadContext<'store>) -> &'a VersionedCell<'store, T>,
// {
//     pub fn with<F, R>(&self, f: F) -> R
//     where
//         F: for<'a, 'store> Fn(&'a VersionedCell<'store, T>, ReadContext<'store>) -> R,
//     {
//         self.shared
//             .with(|root, cx| f((self.select)(root, cx), cx))
//     }
// }
//
// pub struct OnOptionSelector<S> {
//     select: S,
//     shared: Arc<Shared>,
//     last_version: Option<u64>,
//     on_update: OnUpdate,
// }
//
// impl<S, T> OnOptionSelector<S>
// where
//     S: for<'a, 'store> Fn(
//         &'a Root<'store>,
//         ReadContext<'store>,
//     ) -> Option<&'a VersionedCell<'store, T>>,
// {
//     fn new(store: &Store, f: S) -> Self {
//         let last_version = store.with(|root, cx| f(root, cx).map(|v| v.version()));
//
//         OnOptionSelector {
//             shared: store.shared.clone(),
//             select: f,
//             last_version,
//             on_update: store.on_update(),
//         }
//     }
// }
//
// impl<S, T> Stream for OnOptionSelector<S>
// where
//     S: for<'a, 'store> Fn(
//             &'a Root<'store>,
//             ReadContext<'store>,
//         ) -> Option<&'a VersionedCell<'store, T>>
//         + Clone,
// {
//     type Item = OptionSelector<S>;
//
//     fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         let OnOptionSelector {
//             select,
//             shared,
//             last_version,
//             on_update,
//         } = unsafe { self.get_unchecked_mut() };
//
//         match Pin::new(on_update).poll_next(cx) {
//             Poll::Ready(Some(_)) => {
//                 let version = shared.with(|root, cx| select(root, cx).map(|v| v.version()));
//
//                 if version != *last_version {
//                     *last_version = version;
//
//                     Poll::Ready(Some(OptionSelector {
//                         select: select.clone(),
//                         shared: shared.clone(),
//                     }))
//                 } else {
//                     Poll::Pending
//                 }
//             }
//             Poll::Ready(None) => Poll::Ready(None),
//             Poll::Pending => Poll::Pending,
//         }
//     }
// }
//
// pub struct OptionSelector<S> {
//     select: S,
//     shared: Arc<Shared>,
// }
//
// impl<S, T> OptionSelector<S>
// where
//     S: for<'a, 'store> Fn(
//         &'a Root<'store>,
//         ReadContext<'store>,
//     ) -> Option<&'a VersionedCell<'store, T>>,
// {
//     pub fn with<F, R>(&self, f: F) -> R
//     where
//         F: Fn(Option<&T>) -> R,
//     {
//         self.shared
//             .with(|root, cx| f((self.select)(root, cx).map(|v| v.deref(cx))))
//     }
// }
//
// pub trait IterSelect<'a, 'store: 'a, T: 'a>:
//     Fn(&'a Root<'store>, ReadContext<'store>) -> <Self as IterSelect<'a, 'store, T>>::IntoIter
// {
//     type IntoIter: IntoIterator<Item = &'a VersionedCell<'store, T>>;
// }
//
// impl<'a, 'store: 'a, T: 'a, F, I> IterSelect<'a, 'store, T> for F
// where
//     F: Fn(&'a Root<'store>, ReadContext<'store>) -> I,
//     I: IntoIterator<Item = &'a VersionedCell<'store, T>>,
// {
//     type IntoIter = I;
// }
//
// pub struct OnIterSelector<S, T> {
//     select: S,
//     shared: Arc<Shared>,
//     last_version: u64,
//     on_update: OnUpdate,
//     _marker: marker::PhantomData<*const T>,
// }
//
// impl<S, T> OnIterSelector<S, T>
// where
//     S: for<'a, 'store> IterSelect<'a, 'store, T>,
// {
//     fn new(store: &Store, f: S) -> Self {
//         let last_version = store.with(|root, cx| {
//             let mut hasher = SeaHasher::new();
//
//             for element in f(root, cx).into_iter() {
//                 element.version().hash(&mut hasher)
//             }
//
//             hasher.finish()
//         });
//
//         OnIterSelector {
//             shared: store.shared.clone(),
//             select: f,
//             last_version,
//             on_update: store.on_update(),
//             _marker: marker::PhantomData,
//         }
//     }
// }
//
// pub struct VersionedCellIter<'store, I> {
//     inner: I,
//     cx: ReadContext<'store>,
// }
//
// impl<'store, 'a, I, T> Iterator for VersionedCellIter<'store, I>
// where
//     I: Iterator<Item = &'a VersionedCell<'store, T>>,
//     T: 'a,
//     'store: 'a,
// {
//     type Item = &'a T;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         self.inner.next().map(|cell| cell.deref(self.cx))
//     }
// }
//
// pub struct IterSelector<S, T> {
//     select: S,
//     shared: Arc<Shared>,
//     _marker: marker::PhantomData<*const T>,
// }
//
// impl<S, T> IterSelector<S, T>
// where
//     S: for<'a, 'store> IterSelect<'a, 'store, T>,
// {
//     pub fn with<F, R>(&self, f: F) -> R
//     where
//         F: for<'a, 'store> Fn(
//             VersionedCellIter<
//                 'store,
//                 <<S as IterSelect<
//                     'a,
//                     'store,
//                     T,
//                 >>::IntoIter as IntoIterator>::IntoIter,
//             >,
//         ) -> R,
//     {
//         self.shared.with(|root, cx| {
//             f(VersionedCellIter {
//                 inner: (self.select)(root, cx).into_iter(),
//                 cx,
//             })
//         })
//     }
// }

fn asdf() {
    let store_a = Store::initialize(|cx| Root {
        element: VersionedCell::new(
            cx,
            LeafElement {
                a: 0,
                b: "".to_string(),
            },
        ),
        element2: VersionedCell::new(
            cx,
            NodeElement {
                a: 0,
                element: None,
            },
        ),
        leaf_elements: vec![],
        node_elements: vec![],
    });

    let store_b = Store::initialize(|cx| Root {
        element: VersionedCell::new(
            cx,
            LeafElement {
                a: 0,
                b: "".to_string(),
            },
        ),
        element2: VersionedCell::new(
            cx,
            NodeElement {
                a: 0,
                element: None,
            },
        ),
        leaf_elements: vec![],
        node_elements: vec![],
    });

    store_a.with(|root_a, cx_a| {
        store_b.with(|root_b, cx_b| {
            root_b.element.deref(cx_b);
        })
    });

    let leaf_view = LeafView::new(&store_a, |root, cx| &root.element);

    let node_element_view = NodeElementView::new(&store_a, |root: &Root, cx| &root.element2);

    let leaf_slice_view = LeaveSliceView::new(&store_a, |root, cx| &root.leaf_elements);

    let node_element_slice_view =
        NodeElementSliceView::new(&store_a, |root, cx| &root.node_elements);
    //
    // store_a.on_option_selector(|root, cx| root.elements.get(0));

    // store_a.on_iter_selector(|root: &Root, cx| root.elements.iter());
}
