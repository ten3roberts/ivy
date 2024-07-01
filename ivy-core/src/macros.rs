#![allow(non_snake_case)]

#[macro_export]
macro_rules! expand {
    ($macro:ident, $letter:ident) => {
        //$macro!($letter);
    };
    ($macro:ident, $letter:ident, $($tail:ident),*) => {
        $macro!($letter, $($tail),*);
        $crate::expand!($macro, $($tail),*);
    };
}

#[macro_export]
macro_rules! impl_for_tuples {
    ($macro:ident) => {
        $crate::expand!($macro, O, N, M, L, K, J, I, H, G, F, E, D, C, B, A);
    };
}

#[macro_export]
/// Returns the length of a tuple
macro_rules! tuple_len {
    () => {0usize};
    ($head:tt $($tail:tt)*) => {1usize + $crate::tuple_len!($($tail)*)};
}
