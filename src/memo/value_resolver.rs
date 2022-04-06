use crate::store::ReadContext;
use crate::TypeConstructor;

pub trait ValueResolver {
    type RootTC: TypeConstructor;

    type Value<'a, 'store: 'a>;

    fn select<'a, 'store>(
        &self,
        root: &'a <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> Self::Value<'a, 'store>;
}
