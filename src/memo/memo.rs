use std::ops::Deref;

use crate::TypeConstructor;
use crate::memo::Selector;
use crate::store::ReadContext;

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

    type Target<'store>;

    type Selector: for<'store> Selector<RootTC = Self::RootTC, Target<'store> = Self::Target<'store>>;

    fn refresh<'a, 'store>(
        &mut self,
        root: &'a <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<&'a Self::Target<'store>>;

    fn selector(&self) -> Self::Selector;
}
