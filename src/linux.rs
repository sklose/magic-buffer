use crate::BufferError;

#[derive(Debug)]
pub struct VoodooBuffer {
    _addr: *mut u8,
    len: usize,
}

impl VoodooBuffer {
    pub fn new(len: usize) -> Result<Self, BufferError> {
        if !len.is_power_of_two() {
            return Err(BufferError {
                msg: "len must be power of two".to_string(),
            });
        }

        // https://lo.calho.st/posts/black-magic-buffer/
        todo!()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub(crate) unsafe fn as_slice(&self, _offset: usize, _len: usize) -> &[u8] {
        todo!()
    }

    #[inline(always)]
    pub(crate) unsafe fn as_slice_mut(&mut self, _offset: usize, _len: usize) -> &mut [u8] {
        todo!()
    }
}
