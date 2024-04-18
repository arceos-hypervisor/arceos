use core::mem::size_of;
use core::slice::{from_raw_parts, from_raw_parts_mut};

/// A trait bound defined for types which are safe to convert to a byte slice and
/// to create from a byte slice.
pub trait ByteCode: Default + Copy + Send + Sync {
    /// Return the contents of an object (impl trait `ByteCode`) as a slice of bytes.
    /// the inverse of this function is "from_bytes"
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: The object is guaranteed been initialized already.
        unsafe { from_raw_parts(self as *const Self as *const u8, size_of::<Self>()) }
    }

    /// Return the contents of a mutable object (impl trait `ByteCode`) to a mutable slice of bytes.
    /// the inverse of this function is "from_bytes_mut"
    fn as_mut_bytes(&mut self) -> &mut [u8] {
        // SAFETY: The object is guaranteed been initialized already.
        unsafe { from_raw_parts_mut(self as *mut Self as *mut u8, size_of::<Self>()) }
    }

    /// Creates an object (impl trait `ByteCode`) from a slice of bytes
    ///
    /// # Arguments
    ///
    /// * `data` - the slice of bytes that will be constructed as an object.
    fn from_bytes(data: &[u8]) -> Option<&Self> {
        if data.len() != size_of::<Self>() {
            return None;
        }

        // SAFETY: The pointer is properly aligned and point to an initialized instance of T.
        unsafe { data.as_ptr().cast::<Self>().as_ref() }
    }

    /// Creates an mutable object (impl trait `ByteCode`) from a mutable slice of bytes
    ///
    /// # Arguments
    ///
    /// * `data` - the slice of bytes that will be constructed as an mutable object.
    fn from_mut_bytes(data: &mut [u8]) -> Option<&mut Self> {
        if data.len() != size_of::<Self>() {
            return None;
        }

        // SAFETY: The pointer is properly aligned and point to an initialized instance of T.
        unsafe { data.as_mut_ptr().cast::<Self>().as_mut() }
    }
}

// Integer types of Rust satisfy the requirements of `trait ByteCode`
impl ByteCode for usize {}
impl ByteCode for u8 {}
impl ByteCode for u16 {}
impl ByteCode for u32 {}
impl ByteCode for u64 {}
impl ByteCode for u128 {}
impl ByteCode for isize {}
impl ByteCode for i8 {}
impl ByteCode for i16 {}
impl ByteCode for i32 {}
impl ByteCode for i64 {}
impl ByteCode for i128 {}
