mod location;
mod r#move;
mod pieces;
pub mod score;
mod side;
pub use location::*;
pub use r#move::*;
pub use pieces::*;
pub use score::{PackedScore, Score};
pub use side::*;

macro_rules! impl_from_repr {
    ($name:ident) => {
        impl $name {
            #[inline]
            pub const fn from_repr(repr: u8) -> Option<Self> {
                if repr < Self::COUNT as u8 {
                    Some(unsafe { std::mem::transmute::<u8, $name>(repr) })
                } else {
                    None
                }
            }

            /// # Safety
            /// This will only be valid if [`repr`] < [`Self::COUNT`]
            #[inline]
            pub const unsafe fn from_repr_unchecked(repr: u8) -> Self {
                unsafe { std::mem::transmute(repr) }
            }
        }
    };
}

macro_rules! impl_index {
    ($name:ident) => {
        impl<T> std::ops::Index<$name> for [T; $name::COUNT] {
            type Output = T;

            fn index(&self, index: $name) -> &Self::Output {
                unsafe { self.get_unchecked(index as usize) }
            }
        }

        impl<T> std::ops::IndexMut<$name> for [T; $name::COUNT] {
            fn index_mut(&mut self, index: $name) -> &mut Self::Output {
                unsafe { self.get_unchecked_mut(index as usize) }
            }
        }
    };
}

pub(in crate::core::types) use impl_from_repr;
pub(in crate::core::types) use impl_index;
