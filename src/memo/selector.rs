use crate::TypeConstructor;
use crate::store::ReadContext;

pub trait Selector {
    type RootTC: TypeConstructor;

    type Target<'store>;

    fn select<'a, 'store>(
        &self,
        root: &'a <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> &'a Self::Target<'store>;
}
