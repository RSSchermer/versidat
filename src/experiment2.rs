use crate::on_update::{OnUpdate, UpdateBroadcaster};
use crate::{ReadContext, UpdateContext, UpdateContextProvider, VersionedCell};
use std::marker;
use std::sync::{Arc, RwLock};
use seahash::SeaHasher;
use std::hash::{Hash, Hasher};
use std::borrow::Borrow;
use std::marker::PhantomData;




/*
pub struct NodeView<N, C, S>
    where
        C: TypeConstructor,
{
    select: S,
    lock: Arc<Lock<C>>,
    last_version: u64,
    on_update: OnUpdate,
    _marker: PhantomData<N>
}

impl<S, C, N> NodeView<N, C, S>
    where
        C: TypeConstructor,
        N: TypeConstructor,
        S: for<'a, 'store> Fn(
            &'a <C as TypeConstructor>::Type<'store>,
            ReadContext<'store>,
        ) -> &'a VersionedCell<'store, N::Type<'store>>,
{
    pub fn new(store: &Store<C>, select: S) -> Self {
        let last_version = store.with(|root, cx| select(root, cx).version());

        NodeView {
            select,
            lock: store.lock.clone(),
            last_version,
            on_update: store.on_update(),
            _marker: Default::default()
        }
    }

    pub fn with<F, R>(&self, f: F) -> R
        where
            F: for<'store> FnOnce(&N::Type<'store>, ReadContext<'store>) -> R,
    {
        self.lock
            .with(|root, cx| f((self.select)(root, cx).deref(cx), cx))
    }
}

pub struct OptionView<C, S>
where
    C: TypeConstructor,
{
    select: S,
    lock: Arc<Lock<C>>,
    last_version: Option<u64>,
    on_update: OnUpdate,
}

impl<S, C, T> OptionView<C, S>
where
    C: TypeConstructor,
    S: for<'a, 'store> Fn(
        &'a <C as TypeConstructor>::Type<'store>,
        ReadContext<'store>,
    ) -> Option<&'a VersionedCell<'store, T>>,
{
    pub fn new(store: &Store<C>, select: S) -> Self {
        let last_version = store.with(|root, cx| select(root, cx).map(|c| c.version()));

        OptionView {
            select,
            lock: store.lock.clone(),
            last_version,
            on_update: store.on_update(),
        }
    }

    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Option<&T>) -> R,
    {
        self.lock
            .with(|root, cx| f((self.select)(root, cx).map(|c| c.deref(cx))))
    }
}

// TODO: I'd really like a view that generalizes over any `IntoIterator<Item=&VersionedCell<T>>`,
// but I've not been able to find anything that compiles, so use a specific `SliceView` for now so
// that at least slices and `Vec`s work.

pub struct SliceView<C, S>
    where
        C: TypeConstructor,
{
    select: S,
    lock: Arc<Lock<C>>,
    last_versions_hash: u64,
    on_update: OnUpdate,
}

impl<S, C, T> SliceView<C, S>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(
            &'a <C as TypeConstructor>::Type<'store>,
            ReadContext<'store>,
        ) -> &'a [VersionedCell<'store, T>],
{
    pub fn new(store: &Store<C>, select: S) -> Self {
        let last_versions_hash = store.with(|root, cx| {
            let mut hasher = SeaHasher::new();

             for cell in select(root, cx) {
                 cell.version().hash(&mut hasher);
             }

            hasher.finish()
        });

        SliceView {
            select,
            lock: store.lock.clone(),
            last_versions_hash,
            on_update: store.on_update(),
        }
    }

    pub fn with<F, R>(&self, f: F) -> R
        where
            F: for<'store> FnOnce(&[VersionedCell<'store, T>], ReadContext<'store>) -> R,
    {
        self.lock
            .with(|root, cx| f((self.select)(root, cx), cx))
    }
}

pub struct OptionSliceView<C, S>
    where
        C: TypeConstructor,
{
    select: S,
    lock: Arc<Lock<C>>,
    last_versions_hash: Option<u64>,
    on_update: OnUpdate,
}

impl<S, C, T> OptionSliceView<C, S>
    where
        C: TypeConstructor,
        S: for<'a, 'store> Fn(
            &'a <C as TypeConstructor>::Type<'store>,
            ReadContext<'store>,
        ) -> Option<&'a [VersionedCell<'store, T>]>,
{
    pub fn new(store: &Store<C>, select: S) -> Self {
        let last_versions_hash = store.with(|root, cx| {
            select(root, cx).map(|slice| {
                let mut hasher = SeaHasher::new();

                for cell in slice {
                    cell.version().hash(&mut hasher);
                }

                hasher.finish()
            })
        });

        OptionSliceView {
            select,
            lock: store.lock.clone(),
            last_versions_hash,
            on_update: store.on_update(),
        }
    }

    pub fn with<F, R>(&self, f: F) -> R
        where
            F: for<'store> FnOnce(Option<&[VersionedCell<'store, T>]>, ReadContext<'store>) -> R,
    {
        self.lock
            .with(|root, cx| f((self.select)(root, cx), cx))
    }
}

fn test() {
    struct MyRoot<'store> {
        element: VersionedCell<'store, Element>,
        node_element: VersionedCell<'store, NodeElement<'store>>,
        elements: Vec<VersionedCell<'store, Element>>
    }

    gen_type_constructor!(MyRoot, MyRootTC);

    struct Element {
        a: u32,
    }

    struct NodeElement<'store> {
        element: VersionedCell<'store, Element>
    }

    gen_type_constructor!(NodeElement, NodeElementTC);

    type MyStore = Store<MyRootTC>;

    let store = MyStore::initialize(|cx| MyRoot {
        element: VersionedCell::new(cx, Element { a: 0 }),
        node_element: VersionedCell::new(cx, NodeElement {
            element: VersionedCell::new(cx,Element { a: 1})
        }),
        elements: vec![]
    });

    let a = store.with(|root, cx| root.element.deref(cx).a);

    let view = View::new(&store, |root, cx| &root.element);

    let option_view = OptionView::new(&store, |root, cx| root.elements.get(0));

    let slice_view= SliceView::new(&store, |root, cx| {
        &root.elements
    });

    slice_view.with(|slice, cx| {
        for cell in slice {
            println!("{}", cell.deref(cx).a);
        }
    });

    let option_slice_view= OptionSliceView::new(&store, |root, cx| {
        Some(&root.elements)
    });

    let node_view = NodeView::<NodeElementTC, _, _>::new(&store, |root, cx| {
        &root.node_element
    });

    node_view.with(|node, cx| {
        node.element.deref(cx).a
    });
}

// pub trait Root {
//     type Initializer: StoreInitializer;
// }
//
// pub struct Store<R> where R: Root {
//     root: <<R as Root>::Initializer as StoreInitializer>::Root<'static>
// }
//
// impl<R: Root> Store<R> {
//     pub fn initialize(initializer: R::Initializer) -> Self {
//         let mut update_context_provider = UpdateContextProvider::new();
//
//         let root = unsafe {
//             initializer(update_context_provider.update_context())
//         };
//
//         Store { root }
//     }
// }

// pub trait Store {
//     type Root<'scope>;
// }
//
// pub struct MyStore<R> {
//     root: R
// }

// pub trait Root: Sized {
//     type This<'scope> = Self;
// }
//
// pub struct Store<R>
// where
//     R: Root,
// {
//     root: R::This<'static>,
// }
//
// impl<R> Store<R>
// where
//     R: Root,
// {
//     pub fn initialize<F>(initializer: F) -> Self
//     where
//         F: for<'store> FnOnce(UpdateContext<'store>) -> R::This<'store>,
//     {
//         let mut update_context_provider = UpdateContextProvider::new();
//
//         let root: R::This<'static> = unsafe {
//             initializer(update_context_provider.update_context())
//         };
//
//         Store { root }
//     }
// }
//
// fn test() {
//     struct MyRoot<'store> {
//         element_0: VersionedCell<'store, MyElement>,
//         element_1: VersionedCell<'store, MyElement>,
//     }
//
//     impl Root for MyRoot<'_> {}
//
//     struct MyElement {
//         a: u32
//     }
//
//     let store: Store<MyRoot> = Store::initialize(|cx| {
//         let root: <MyRoot as Root>::This<'_> = MyRoot {
//             element_0: VersionedCell::new(cx, MyElement { a: 0 }),
//             element_1: VersionedCell::new(cx, MyElement { a: 0 }),
//         };
//
//         root
//     });
// }

// pub trait Root: Sized {
//     type This<'scope>;
// }
//
// pub struct Store<R> where R: Root {
//     root: R::This<'static>
// }

// impl<R> Store<R> where R: Root<'static> {
//
// }
*/