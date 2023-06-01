use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[cfg(target_family = "windows")]
mod win;

#[cfg(target_family = "windows")]
pub use win::*;

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

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_BUF_LEN: usize = 1 << 16;
    const INVALID_BUF_LEN_ALIGN: usize = 1 << 8;
    const INVALID_BUF_LEN_POW2: usize = 1 << 16 + 5;

    #[test]
    fn allocates_buffer() {
        let buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        drop(buf);
    }

    #[test]
    fn requires_power_of_two() {
        InfiniteBuffer::new(INVALID_BUF_LEN_POW2)
            .map_err(|e| {
                println!("{}", e.msg);
                e
            })
            .expect_err("should not allocate buffer");
    }

    #[test]
    fn requires_aligned_len() {
        InfiniteBuffer::new(INVALID_BUF_LEN_ALIGN)
            .map_err(|e| {
                println!("{}", e.msg);
                e
            })
            .expect_err("should not allocate buffer");
    }

    #[test]
    fn writes_are_visible_wrap_around() {
        let mut buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        buf[0] = b'a';
        assert_eq!(buf[0], buf[VALID_BUF_LEN]);
    }

    #[test]
    fn deref_as_slice() {
        let buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice: &[u8] = &buf;
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn deref_mut_as_slice() {
        let mut buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice: &mut [u8] = &mut buf;
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn closed_range() {
        let buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[0..VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn closed_range_mut() {
        let mut buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[0..VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to() {
        let buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..VALID_BUF_LEN + 1];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_mut() {
        let mut buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..VALID_BUF_LEN + 1];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_from() {
        let buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[1..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_from_mut() {
        let mut buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[1..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_inclusive() {
        let buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..=VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_to_inclusive_mut() {
        let mut buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..=VALID_BUF_LEN];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_full() {
        let buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &buf[..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }

    #[test]
    fn range_full_mut() {
        let mut buf = InfiniteBuffer::new(VALID_BUF_LEN).expect("should allocate buffer");
        let slice = &mut buf[..];
        assert_eq!(VALID_BUF_LEN, slice.len());
    }
}
