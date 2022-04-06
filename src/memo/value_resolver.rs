use crate::store::ReadContext;
use crate::TypeConstructor;

pub trait ValueResolver {
    type RootTC: TypeConstructor;

    type Value<'store>;

    fn select<'a, 'store>(
        &self,
        root: &'a <Self::RootTC as TypeConstructor>::Type<'store>,
        cx: ReadContext<'store>,
    ) -> &'a Self::Value<'store>;
}
