use crate::store::ReadContext;
use crate::TypeConstructor;

pub struct Refresh<T> {
    pub value: T,
    pub is_changed: bool
}

pub trait Memo {
    type RootTC: TypeConstructor;

    type Value<'a, 'b, 'store: 'b> where Self: 'a;

    fn store_id(&self) -> usize;

    fn refresh_unchecked<'a, 'b, 'store>(
        &'a mut self,
        root: &'b <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<Self::Value<'a, 'b, 'store>>;

    fn refresh<'a, 'b, 'store>(
        &'a mut self,
        root: &'b <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Refresh<Self::Value<'a, 'b, 'store>> {
        if self.store_id() != cx.store_id() {
            panic!(
                "memo is associated with a different store than the read context that was passed"
            );
        }

        self.refresh_unchecked(root, cx)
    }
}
