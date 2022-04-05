pub trait TypeConstructor {
    type Type<'store>: 'store;
}

#[macro_export]
macro_rules! gen_type_constructor {
    ($tpe:ident, $vis:vis $tpe_constructor:ident) => {
        $vis struct $tpe_constructor;

        impl $crate::TypeConstructor for $tpe_constructor {
            type Type<'store> = $tpe<'store>;
        }
    }
}
