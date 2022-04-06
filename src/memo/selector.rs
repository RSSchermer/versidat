use crate::TypeConstructor;
use crate::store::ReadContext;

pub trait Selector {
    type RootTC: TypeConstructor;

    type Target<'a, 'store: 'a>;

    fn select<'a, 'store: 'a>(
        &self,
        root: &'a <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Self::Target<'a, 'store>;
}
