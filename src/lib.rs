use std::{
    error::Error,
    fmt::{Display, Formatter},
    ops::{
        Deref, DerefMut, Index, IndexMut, Range, RangeFrom, RangeFull, RangeTo, RangeToInclusive,
    },
};

#[cfg(target_family = "windows")]
mod windows;

#[cfg(target_family = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod macos;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use macos::*;

#[derive(Debug)]
pub struct BufferError {
    msg: String,
}

impl Display for BufferError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl Error for BufferError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl Deref for VoodooBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { self.as_slice(0, self.len()) }
    }
}

impl DerefMut for VoodooBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.as_slice_mut(0, self.len()) }
    }
}

impl Index<usize> for VoodooBuffer {
    type Output = u8;

    fn index(&self, mut index: usize) -> &Self::Output {
        index %= self.len();
        unsafe { self.as_slice(index, 1).get_unchecked(0) }
    }
}

impl IndexMut<usize> for VoodooBuffer {
    fn index_mut(&mut self, mut index: usize) -> &mut Self::Output {
        index %= self.len();
        unsafe { self.as_slice_mut(index, 1).get_unchecked_mut(0) }
    }
}

impl Index<Range<usize>> for VoodooBuffer {
    type Output = [u8];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        if index.start > index.end {
            return &[];
        }

        let len = index.end - index.start;
        if len > self.len() {
            panic!("out of bounds")
        }

        let index = index.start % self.len();
        unsafe { self.as_slice(index, len) }
    }
}

impl IndexMut<Range<usize>> for VoodooBuffer {
    fn index_mut(&mut self, index: Range<usize>) -> &mut Self::Output {
        if index.start > index.end {
            return &mut [];
        }

        let len = index.end - index.start;
        if len > self.len() {
            panic!("out of bounds")
        }

        let index = index.start % self.len();
        unsafe { self.as_slice_mut(index, len) }
    }
}

impl Index<RangeTo<usize>> for VoodooBuffer {
    type Output = [u8];

    fn index(&self, index: RangeTo<usize>) -> &Self::Output {
        let start = index.end - self.len();
        let index = start % self.len();
        unsafe { self.as_slice(index, self.len()) }
    }
}

impl IndexMut<RangeTo<usize>> for VoodooBuffer {
    fn index_mut(&mut self, index: RangeTo<usize>) -> &mut Self::Output {
        let start = index.end - self.len();
        let index = start % self.len();
        unsafe { self.as_slice_mut(index, self.len()) }
    }
}

impl Index<RangeFrom<usize>> for VoodooBuffer {
    type Output = [u8];

    fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
        let index = index.start % self.len();
        unsafe { self.as_slice(index, self.len()) }
    }
}

impl IndexMut<RangeFrom<usize>> for VoodooBuffer {
    fn index_mut(&mut self, index: RangeFrom<usize>) -> &mut Self::Output {
        let index = index.start % self.len();
        unsafe { self.as_slice_mut(index, self.len()) }
    }
}

impl Index<RangeToInclusive<usize>> for VoodooBuffer {
    type Output = [u8];

    fn index(&self, index: RangeToInclusive<usize>) -> &Self::Output {
        let start = index.end - self.len() + 1;
        let index = start % self.len();
        unsafe { self.as_slice(index, self.len()) }
    }
}

impl IndexMut<RangeToInclusive<usize>> for VoodooBuffer {
    fn index_mut(&mut self, index: RangeToInclusive<usize>) -> &mut Self::Output {
        let start = index.end - self.len() + 1;
        let index = start % self.len();
        unsafe { self.as_slice_mut(index, self.len()) }
    }
}

impl Index<RangeFull> for VoodooBuffer {
    type Output = [u8];

    fn index(&self, _: RangeFull) -> &Self::Output {
        unsafe { self.as_slice(0, self.len()) }
    }
}

impl IndexMut<RangeFull> for VoodooBuffer {
    fn index_mut(&mut self, _: RangeFull) -> &mut Self::Output {
        unsafe { self.as_slice_mut(0, self.len()) }
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
        let buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        drop(buf);
    }

    #[test]
    fn requires_power_of_two() {
        VoodooBuffer::new(INVALID_BUF_LEN_POW2)
            .map_err(|e| {
                println!("{}", e.msg);
                e
            })
            .expect_err("should not allocate buffer");
    }

    #[test]
    fn requires_aligned_len() {
        VoodooBuffer::new(INVALID_BUF_LEN_ALIGN)
            .map_err(|e| {
                println!("{}", e.msg);
                e
            })
            .expect_err("should not allocate buffer");
    }

    #[test]
    fn writes_are_visible_wrap_around() {
        let mut buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        buf[0] = b'a';
        assert_eq!(buf[0], buf[VALID_BUF_LEN]);
    }

    #[test]
    fn deref_as_slice() {
        let buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice: &[u8] = &buf;
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn deref_mut_as_slice() {
        let mut buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice: &mut [u8] = &mut buf;
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn closed_range() {
        let buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[0..VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn closed_range_mut() {
        let mut buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[0..VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to() {
        let buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..VALID_BUF_LEN + 1];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_mut() {
        let mut buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..VALID_BUF_LEN + 1];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_from() {
        let buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[1..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_from_mut() {
        let mut buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[1..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_inclusive() {
        let buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..=VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_inclusive_mut() {
        let mut buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..=VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_full() {
        let buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_full_mut() {
        let mut buf = VoodooBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }
}
