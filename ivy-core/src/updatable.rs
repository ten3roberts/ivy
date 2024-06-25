// #![allow(non_snake_case)]
// pub trait Updatable {
//     fn update(&self);
// }

// macro_rules! tuple_impl {
//     ($($name: ident),*) => {
//         impl<$($name: Updatable),*> Updatable for ($($name,)*) {
//             // Draws the scene using the pass [`Pass`] and the provided camera.
//             // Note: camera must have gpu side data.
//             fn update(&self) {
//                 let ($($name,)+) = self;

//                 ($($name.update()), *);
//             }
//         }
//     }
// }

// // Implement renderer on tuple of renderers and tuple of render handles
// tuple_impl! { A }
// tuple_impl! { A, B }
// tuple_impl! { A, B, C }
// tuple_impl! { A, B, C, D }
// tuple_impl! { A, B, C, D, E }
// tuple_impl! { A, B, C, D, E, F }
// tuple_impl! { A, B, C, D, E, F, G }
// tuple_impl! { A, B, C, D, E, F, G, H }
// tuple_impl! { A, B, C, D, E, F, G, H, I }
// tuple_impl! { A, B, C, D, E, F, G, H, I, J }
// tuple_impl! { A, B, C, D, E, F, G, H, I, J, K }
// tuple_impl! { A, B, C, D, E, F, G, H, I, J, K, L }
