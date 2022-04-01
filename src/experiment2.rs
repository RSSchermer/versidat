use crate::on_update::{OnUpdate, UpdateBroadcaster};
use crate::{ReadContext, UpdateContext, UpdateContextProvider, VersionedCell};
use std::sync::{Arc, RwLock};

pub trait TypeConstructor {
    type Type<'store>;
}

macro_rules! gen_type_constructor {
    ($tpe:ident, $vis:vis $tpe_constructor:ident) => {
        $vis struct $tpe_constructor;

        impl $crate::experiment2::TypeConstructor for $tpe_constructor {
            type Type<'store> = $tpe<'store>;
        }
    }
}

struct Shared<C>
where
    C: TypeConstructor,
{
    data: <C as TypeConstructor>::Type<'static>,
    update_context_provider: UpdateContextProvider,
}

struct Lock<C>
where
    C: TypeConstructor,
{
    shared: RwLock<Shared<C>>,
}

impl<C> Lock<C>
where
    C: TypeConstructor,
{
    fn with<F, R>(&self, f: F) -> R
    where
        F: for<'store> FnOnce(&<C as TypeConstructor>::Type<'store>, ReadContext<'store>) -> R,
    {
        let lock = self.shared.read().expect("poisoned");

        unsafe {
            f(
                ::std::mem::transmute::<&<C as TypeConstructor>::Type<'static>, _>(&lock.data),
                ReadContext::new(),
            )
        }
    }
}

pub struct Store<C>
where
    C: TypeConstructor,
{
    lock: Arc<Lock<C>>,
    update_broadcaster: UpdateBroadcaster,
}

impl<C> Store<C>
where
    C: TypeConstructor,
{
    pub fn initialize<F>(initializer: F) -> Self
    where
        F: for<'store> FnOnce(UpdateContext<'store>) -> <C as TypeConstructor>::Type<'store>,
    {
        let mut update_context_provider = UpdateContextProvider::new();

        let data = unsafe { initializer(update_context_provider.update_context()) };

        let shared = Shared {
            data,
            update_context_provider,
        };

        Store {
            lock: Arc::new(Lock {
                shared: RwLock::new(shared),
            }),
            update_broadcaster: UpdateBroadcaster::new(),
        }
    }

    pub fn with<F, R>(&self, f: F) -> R
    where
        F: for<'store> FnOnce(&<C as TypeConstructor>::Type<'store>, ReadContext<'store>) -> R,
    {
        self.lock.with(f)
    }

    pub fn update<F>(&self, f: F)
    where
        F: for<'store> FnOnce(&<C as TypeConstructor>::Type<'store>, UpdateContext<'store>),
    {
        let mut lock = self.lock.shared.write().expect("poisoned");

        let Shared {
            data,
            update_context_provider,
        } = &mut *lock;

        let result = unsafe {
            f(
                ::std::mem::transmute::<&mut <C as TypeConstructor>::Type<'static>, _>(data),
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

fn test() {
    struct MyRoot<'store> {
        element: VersionedCell<'store, Element>,
    }

    gen_type_constructor!(MyRoot, MyRootTC);

    struct Element {
        a: u32,
    }

    type MyStore = Store<MyRootTC>;

    let store = MyStore::initialize(|cx| {
        MyRoot {
            element: VersionedCell::new(cx, Element {
                a: 0
            })
        }
    });

    let a = store.with(|root, cx| {
        root.element.deref(cx).a
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
