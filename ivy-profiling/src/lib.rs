#[doc(hidden)]
pub mod __internal {
    #[cfg(feature = "profile_with_puffin")]
    pub use puffin;
}

#[cfg(feature = "profile_with_puffin")]
#[macro_export]
macro_rules! profile_function {
    ($($tt: tt)*) => (
        $crate::__internal::puffin::profile_function!($($tt)*);
    )
}

#[cfg(feature = "profile_with_puffin")]
#[macro_export]
macro_rules! profile_scope {
    ($($tt: tt)*) => (
        $crate::__internal::puffin::profile_scope!($($tt)*);
    )
}

#[cfg(not(feature = "profile_with_puffin"))]
#[macro_export]
macro_rules! profile_function {
    ($($tt: tt)*) => {};
}

#[cfg(not(feature = "profile_with_puffin"))]
#[macro_export]
macro_rules! profile_scope {
    ($($tt: tt)*) => {};
}
