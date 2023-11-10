#![doc = include_str!("../README.md")]

use std::{
    ops::{
        Deref, DerefMut, Index, IndexMut, Range, RangeFrom, RangeFull, RangeTo, RangeToInclusive,
    },
    ptr::{slice_from_raw_parts, slice_from_raw_parts_mut},
};
use thiserror::Error;

#[cfg(target_family = "windows")]
mod windows;

#[cfg(target_family = "windows")]
use windows::*;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
use linux::*;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod macos;

#[cfg(any(target_os = "macos", target_os = "ios"))]
use macos::*;

/// The [`MagicBufferError`] error indicates an allocation failure that may be due
/// to resource exhaustion or to something wrong with the given input arguments
/// to [`MagicBuffer::new`].
#[derive(Debug, Error)]
pub enum MagicBufferError {
    /// There is not enough memory available.
    #[error("out of memory")]
    OOM,
    /// The specified buffer length is invalid. See [`MagicBuffer::new`] for more information.
    #[error("invalid buffer len, {msg}")]
    InvalidLen {
        /// Details on why the `len` is invalid.
        msg: String,
    },
}

#[derive(Debug)]
pub struct MagicBuffer {
    addr: *mut u8,
    len: usize,
    mask: usize,
}

// SAFETY: Memory mappings are not tied to a thread, so they can be sent
// across thread boundaries safely.
unsafe impl Send for MagicBuffer {}

// SAFETY: There is no interior mutability.
unsafe impl Sync for MagicBuffer {}

/// [`MagicBuffer`] provides a ring buffer implementation that
/// can deref into a contiguous slice from any offset wrapping
/// around the buffer.
///
/// This is made possible with virtual address mappings.
/// The underlying buffer is mapped twice into virtual memory where
/// the second mapping is adjacent to the first one. The logic
/// for wrapping around the buffer is pushed down to the hardware.
///
/// # Examples
/// ```
/// # use magic_buffer::*;
/// # fn main() -> Result<(), MagicBufferError> {
/// let len = MagicBuffer::min_len();
/// let buf = MagicBuffer::new(len)?;
/// let slice = &buf[len/2..];
/// assert_eq!(len, slice.len());
/// # Ok(())
/// # }
/// ```
#[allow(clippy::len_without_is_empty)]
impl MagicBuffer {
    /// Allocates a new [`MagicBuffer`] of the specified `len`.
    ///
    /// `len` must be a power of two, and also must be a multiple
    /// of the operating system's allocation granularity. This is
    /// usually the page size - most commonly 4KiB. On Windows
    /// the allocation granularity is 64KiB (see [here](https://devblogs.microsoft.com/oldnewthing/20031008-00/?p=42223)).
    ///
    /// ## Errors
    /// Will return a [`MagicBufferError`] if the allocation fails.
    /// ```rust
    /// # use magic_buffer::{MagicBuffer, MagicBufferError};
    /// let err = MagicBuffer::new(0).unwrap_err();
    /// assert!(matches!(err, MagicBufferError::InvalidLen{ .. }));
    /// ```
    ///
    /// ## Panics
    /// Will panic if it fails to cleanup in case of an error.
    pub fn new(len: usize) -> Result<Self, MagicBufferError> {
        if len == 0 {
            return Err(MagicBufferError::InvalidLen {
                msg: "len must be greater than 0".to_string(),
            });
        }

        if !len.is_power_of_two() {
            return Err(MagicBufferError::InvalidLen {
                msg: "len must be power of two".to_string(),
            });
        }

        let min_len = Self::min_len();
        if len % min_len != 0 {
            return Err(MagicBufferError::InvalidLen {
                msg: format!("len must be page aligned, {}", min_len),
            });
        }

        Ok(Self {
            addr: unsafe { magic_buf_alloc(len) }?,
            mask: len - 1,
            len,
        })
    }

    /// Returns the minimum buffer len that can be allocated.
    ///
    /// This is usually the page size - most commonly 4KiB. On Windows
    /// the allocation granularity is 64KiB (see [here](https://devblogs.microsoft.com/oldnewthing/20031008-00/?p=42223)).
    pub fn min_len() -> usize {
        unsafe { magic_buf_min_len() }
    }

    /// Returns the length of this [`MagicBuffer`].
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns an unsafe pointer to the [`MagicBuffer`]. The `offset` species the first
    /// element the pointer points to. The pointer can be used to address up to `len` elements.
    ///
    /// The caller must ensure that the [`MagicBuffer`] outlives the pointer this function returns,
    /// or else it will end up pointing to garbage.
    ///
    /// The caller must also ensure that the memory the pointer (non-transitively) points to is
    /// never written to (except inside an UnsafeCell) using this pointer or any pointer derived
    /// from it. If you need to mutate the contents of the slice, use [`as_mut_ptr`](MagicBuffer::as_mut_ptr).
    ///
    /// ## Examples
    /// ```rust
    /// # use magic_buffer::MagicBuffer;
    /// let x = MagicBuffer::new(MagicBuffer::min_len()).unwrap();
    /// let x_ptr = x.as_ptr(1);
    ///
    /// unsafe {
    ///     for i in 0..x.len() {
    ///         assert_eq!(*x_ptr.add(i), 0);
    ///     }
    /// }
    /// ```
    pub fn as_ptr(&self, offset: usize) -> *const u8 {
        unsafe { self.addr.add(self.fast_mod(offset)).cast_const() }
    }

    /// Returns an unsafe mutable pointer to the [`MagicBuffer`]. The `offset` species the first
    /// element the mutable pointer points to. The mutable pointer can be used to address up
    /// to `len` elements.
    ///
    /// The caller must ensure that the [`MagicBuffer`] outlives the pointer this function returns,
    /// or else it will end up pointing to garbage.
    ///
    /// ## Examples
    /// ```rust
    /// # use magic_buffer::MagicBuffer;
    /// let mut x = MagicBuffer::new(MagicBuffer::min_len()).unwrap();
    /// let x_ptr = x.as_mut_ptr(1);
    ///
    /// unsafe {
    ///     for i in 0..x.len() {
    ///         *x_ptr.add(i) = (i % 256) as u8;
    ///     }
    /// }
    /// ```
    pub fn as_mut_ptr(&mut self, offset: usize) -> *mut u8 {
        unsafe { self.addr.add(self.fast_mod(offset)) }
    }

    #[inline(always)]
    unsafe fn as_slice(&self, offset: usize, len: usize) -> &[u8] {
        &*(slice_from_raw_parts(self.addr.add(offset), len))
    }

    #[inline(always)]
    unsafe fn as_slice_mut(&mut self, offset: usize, len: usize) -> &mut [u8] {
        &mut *(slice_from_raw_parts_mut(self.addr.add(offset), len))
    }

    #[inline(always)]
    fn fast_mod(&self, v: usize) -> usize {
        v & self.mask
    }
}

impl Drop for MagicBuffer {
    fn drop(&mut self) {
        unsafe { magic_buf_free(self.addr, self.len) }
    }
}

impl Deref for MagicBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { self.as_slice(0, self.len) }
    }
}

impl DerefMut for MagicBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.as_slice_mut(0, self.len) }
    }
}

impl Index<usize> for MagicBuffer {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*self.addr.add(self.fast_mod(index)) }
    }
}

impl IndexMut<usize> for MagicBuffer {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut *self.addr.add(self.fast_mod(index)) }
    }
}

macro_rules! index_impl {
    ($from:ty, $to:ty) => {
        impl Index<$from> for MagicBuffer {
            type Output = u8;

            fn index(&self, index: $from) -> &Self::Output {
                &self[index as $to]
            }
        }

        impl IndexMut<$from> for MagicBuffer {
            fn index_mut(&mut self, index: $from) -> &mut Self::Output {
                &mut self[index as $to]
            }
        }
    };
}

index_impl!(i64, isize);
index_impl!(i32, isize);
index_impl!(i16, isize);
index_impl!(i8, isize);

index_impl!(u64, usize);
index_impl!(u32, usize);
index_impl!(u16, usize);
index_impl!(u8, usize);

impl Index<isize> for MagicBuffer {
    type Output = u8;

    fn index(&self, index: isize) -> &Self::Output {
        let index = if index < 0 {
            self.len - self.fast_mod((-index) as usize)
        } else {
            self.fast_mod(index as usize)
        };
        unsafe { &*self.addr.add(index) }
    }
}

impl IndexMut<isize> for MagicBuffer {
    fn index_mut(&mut self, index: isize) -> &mut Self::Output {
        let index = if index < 0 {
            self.len - self.fast_mod((-index) as usize)
        } else {
            self.fast_mod(index as usize)
        };
        unsafe { &mut *self.addr.add(index) }
    }
}

impl Index<Range<usize>> for MagicBuffer {
    type Output = [u8];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        if index.start > index.end {
            return &[];
        }

        let len = index.end - index.start;
        if len > self.len {
            panic!("out of bounds")
        }

        unsafe { self.as_slice(self.fast_mod(index.start), len) }
    }
}

impl IndexMut<Range<usize>> for MagicBuffer {
    fn index_mut(&mut self, index: Range<usize>) -> &mut Self::Output {
        if index.start > index.end {
            return &mut [];
        }

        let len = index.end - index.start;
        if len > self.len {
            panic!("out of bounds")
        }

        unsafe { self.as_slice_mut(self.fast_mod(index.start), len) }
    }
}

impl Index<RangeTo<usize>> for MagicBuffer {
    type Output = [u8];

    fn index(&self, index: RangeTo<usize>) -> &Self::Output {
        let start = index.end - self.len;
        unsafe { self.as_slice(self.fast_mod(start), self.len) }
    }
}

impl IndexMut<RangeTo<usize>> for MagicBuffer {
    fn index_mut(&mut self, index: RangeTo<usize>) -> &mut Self::Output {
        let start = index.end - self.len;
        unsafe { self.as_slice_mut(self.fast_mod(start), self.len) }
    }
}

impl Index<RangeFrom<usize>> for MagicBuffer {
    type Output = [u8];

    fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
        unsafe { self.as_slice(self.fast_mod(index.start), self.len) }
    }
}

impl IndexMut<RangeFrom<usize>> for MagicBuffer {
    fn index_mut(&mut self, index: RangeFrom<usize>) -> &mut Self::Output {
        unsafe { self.as_slice_mut(self.fast_mod(index.start), self.len) }
    }
}

impl Index<RangeToInclusive<usize>> for MagicBuffer {
    type Output = [u8];

    fn index(&self, index: RangeToInclusive<usize>) -> &Self::Output {
        let start = index.end - self.len + 1;
        unsafe { self.as_slice(self.fast_mod(start), self.len) }
    }
}

impl IndexMut<RangeToInclusive<usize>> for MagicBuffer {
    fn index_mut(&mut self, index: RangeToInclusive<usize>) -> &mut Self::Output {
        let start = index.end - self.len + 1;
        unsafe { self.as_slice_mut(self.fast_mod(start), self.len) }
    }
}

impl Index<RangeFull> for MagicBuffer {
    type Output = [u8];

    fn index(&self, _: RangeFull) -> &Self::Output {
        unsafe { self.as_slice(0, self.len) }
    }
}

impl IndexMut<RangeFull> for MagicBuffer {
    fn index_mut(&mut self, _: RangeFull) -> &mut Self::Output {
        unsafe { self.as_slice_mut(0, self.len) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_BUF_LEN: usize = 1 << 16;
    const INVALID_BUF_LEN_ALIGN: usize = 1 << 8;
    const INVALID_BUF_LEN_POW2: usize = (1 << 16) + 5;

    #[test]
    fn allocates_buffer() {
        let buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        drop(buf);
    }

    #[test]
    fn requires_power_of_two() {
        MagicBuffer::new(INVALID_BUF_LEN_POW2)
            .map_err(|e| {
                println!("{}", e);
                e
            })
            .expect_err("should not allocate buffer");
    }

    #[test]
    fn requires_aligned_len() {
        MagicBuffer::new(INVALID_BUF_LEN_ALIGN)
            .map_err(|e| {
                println!("{}", e);
                e
            })
            .expect_err("should not allocate buffer");
    }

    #[test]
    fn writes_are_visible_wrap_around() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        buf[0] = b'a';
        assert_eq!(buf[0], buf[VALID_BUF_LEN]);
    }

    #[test]
    fn deref_as_slice() {
        let buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice: &[u8] = &buf;
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn deref_mut_as_slice() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice: &mut [u8] = &mut buf;
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn closed_range() {
        let buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[0..VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn closed_range_mut() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[0..VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to() {
        let buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..VALID_BUF_LEN + 1];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_mut() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..VALID_BUF_LEN + 1];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_from() {
        let buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[1..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_from_mut() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[1..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_inclusive() {
        let buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..=VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_inclusive_mut() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..=VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_full() {
        let buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_full_mut() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn index_wrap_around() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        buf[0] = b'1';
        assert_eq!(b'1', buf[VALID_BUF_LEN]);
    }

    #[test]
    fn index_negative() {
        let mut buf = MagicBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        buf[-1] = b'2';
        assert_eq!(b'2', buf[VALID_BUF_LEN - 1]);
    }
}
