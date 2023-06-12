// This implementation is based on
// https://github.com/gnzlbg/slice_deque/blob/master/src/mirrored/linux.rs

use crate::MagicBufferError;

use libc::{
    c_char, c_int, c_long, c_uint, close, ftruncate, mkstemp, mmap, munmap, off_t, size_t, syscall,
    sysconf, unlink, SYS_memfd_create, ENOSYS, MAP_FAILED, MAP_FIXED, MAP_SHARED, PROT_READ,
    PROT_WRITE, _SC_PAGESIZE,
};
use std::ptr;

#[cfg(any(target_os = "android", target_os = "openbsd"))]
use libc::__errno;

#[cfg(not(any(target_os = "android", target_os = "openbsd")))]
use libc::__errno_location;

#[cfg(not(target_os = "openbsd"))]
fn memfd_create(name: *const c_char, flags: c_uint) -> c_long {
    unsafe { syscall(SYS_memfd_create, name, flags) }
}

#[cfg(target_os = "openbsd")]
fn memfd_create(_name: *mut c_char, _flags: c_uint) -> c_long {
    unsafe { *__errno() = ENOSYS };
    return -1;
}

fn errno() -> c_int {
    #[cfg(not(any(target_os = "android", target_os = "openbsd")))]
    unsafe {
        *__errno_location()
    }
    #[cfg(any(target_os = "android", target_os = "openbsd"))]
    unsafe {
        *__errno()
    }
}

pub(super) unsafe fn magic_buf_min_len() -> usize {
    sysconf(_SC_PAGESIZE) as _
}

pub(super) unsafe fn magic_buf_alloc(len: usize) -> Result<*mut u8, MagicBufferError> {
    let file_name = *b"magic_buffer\0";
    let mut fd = memfd_create(file_name.as_ptr() as _, 0);

    if fd == -1 && errno() == ENOSYS {
        // memfd_create is not implemented, use mkstemp instead:
        fd = c_long::from(mkstemp(file_name.as_ptr() as _));
        // and unlink the file
        if fd != -1 {
            assert_eq!(0, unlink(file_name.as_ptr() as _));
        }
    }

    if fd == -1 {
        return Err(MagicBufferError::OOM);
    }

    let fd = fd as c_int;
    if ftruncate(fd, len as off_t) == -1 {
        assert_eq!(0, close(fd));
        return Err(MagicBufferError::OOM);
    };

    // mmap memory
    let ptr = mmap(
        ptr::null_mut(),
        len * 2,
        PROT_READ | PROT_WRITE,
        MAP_SHARED,
        fd,
        0,
    );

    if ptr == MAP_FAILED {
        assert_eq!(0, close(fd));
        return Err(MagicBufferError::OOM);
    }

    let ptr2 = mmap(
        (ptr as *mut u8).add(len) as _,
        len,
        PROT_READ | PROT_WRITE,
        MAP_SHARED | MAP_FIXED,
        fd,
        0,
    );

    if ptr2 == MAP_FAILED {
        assert_eq!(0, munmap(ptr, (len * 2) as size_t));
        assert_eq!(0, close(fd));
        return Err(MagicBufferError::OOM);
    }

    assert_eq!(0, close(fd));
    Ok(ptr as *mut u8)
}

pub(super) unsafe fn magic_buf_free(addr: *mut u8, len: usize) {
    assert_eq!(0, munmap(addr as _, (len * 2) as size_t));
}
