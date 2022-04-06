use std::ops::Deref;

use crate::memo::ValueResolver;
use crate::store::ReadContext;
use crate::TypeConstructor;

pub enum Refresh<T> {
    Unchanged(T),
    Changed(T),
}

impl<T> Refresh<T> {
    pub fn is_changed(&self) -> bool {
        if let Refresh::Changed(_) = self {
            true
        } else {
            false
        }
    }
}

impl<T> Deref for Refresh<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Refresh::Unchanged(s) => s,
            Refresh::Changed(s) => s,
        }
    }
}

pub trait Memo {
    type RootTC: TypeConstructor;

    type Value<'store>;

    type ValueResolver: for<'store> ValueResolver<
        RootTC = Self::RootTC,
        Value<'store> = Self::Value<'store>,
    >;

    fn store_id(&self) -> usize;

    fn refresh_unchecked<'a, 'store>(
        &mut self,
        root: &'a <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<&'a Self::Value<'store>>;

    fn refresh<'a, 'store>(
        &mut self,
        root: &'a <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<&'a Self::Value<'store>> {
        if self.store_id() != cx.store_id() {
            panic!(
                "memo is associated with a different store than the read context that was passed"
            );
        }

        self.refresh_unchecked(root, cx)
    }

    fn value_resolver(&self) -> Self::ValueResolver;
}
