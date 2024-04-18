pub mod byte_code;
pub mod num_ops;

pub mod errors;

use core::any::Any;
/// This trait is to cast trait object to struct.
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Macro: Calculate offset of specified field in a type.
#[macro_export]
macro_rules! __offset_of {
    ($type_name:ty, $field:ident) => {{
        let tmp = core::mem::MaybeUninit::<$type_name>::uninit();
        let outer = tmp.as_ptr();
        // SAFETY: The pointer is valid and aligned, just not initialised; `addr_of` ensures
        // that we don't actually read from `outer` (which would be UB) nor create an
        // intermediate reference.
        let inner = unsafe { core::ptr::addr_of!((*outer).$field) } as *const u8;
        // SAFETY: Two pointers are within the same allocation block.
        unsafe { inner.offset_from(outer as *const u8) as usize }
    }};
}

/// Macro: Calculate offset of a field in a recursive type.
///
/// # Arguments
///
/// The Arguments is: a type name and its field name,
/// follows by a series of sub-type's name and its field's name.
///
/// # Examples
///
/// ```rust
/// #[macro_use]
/// extern crate util;
///
/// fn main() {
///     struct Rectangle {
///         pub length: u64,
///         pub width: u64,
///     }
///     assert_eq!(offset_of!(Rectangle, length), 0);
///     assert_eq!(offset_of!(Rectangle, width), 8);
/// }
/// ```
#[macro_export]
macro_rules! offset_of {
    ($type_name:ty, $field:ident) => { $crate::__offset_of!($type_name, $field) };
    ($type_name:ty, $field:ident, $($sub_type_name:ty, $sub_field:ident), +) => {
        $crate::__offset_of!($type_name, $field) + offset_of!($($sub_type_name, $sub_field), +)
    };
}
