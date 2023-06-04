use crate::BufferError;

pub(super) unsafe fn voodoo_buf_min_len() -> usize {
    todo!()
}

pub(super) unsafe fn voodoo_buf_alloc(len: usize) -> Result<*mut u8, BufferError> {
    todo!()
}

pub(super) unsafe fn voodoo_buf_free(add: *mut u8, len: usize) {
    todo!()
}
