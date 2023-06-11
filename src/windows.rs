// This implementation is based on
// https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualalloc2

use crate::VoodooBufferError;

use std::cmp::max;
use std::{mem::MaybeUninit, ptr};

use windows_sys::core::PWSTR;
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, FALSE, INVALID_HANDLE_VALUE},
    System::{
        Memory::{
            CreateFileMappingA, LocalFree, MapViewOfFile3, UnmapViewOfFile, VirtualAlloc2,
            VirtualFree, MEM_PRESERVE_PLACEHOLDER, MEM_RELEASE, MEM_REPLACE_PLACEHOLDER,
            MEM_RESERVE, MEM_RESERVE_PLACEHOLDER, PAGE_NOACCESS, PAGE_READWRITE,
        },
        SystemInformation::{self, SYSTEM_INFO},
    },
};

pub(super) unsafe fn voodoo_buf_min_len() -> usize {
    let mut sys_info = MaybeUninit::<SYSTEM_INFO>::zeroed();
    SystemInformation::GetSystemInfo(sys_info.as_mut_ptr());
    let sys_info = sys_info.assume_init();

    max(sys_info.dwPageSize, sys_info.dwAllocationGranularity) as usize
}

pub(super) unsafe fn voodoo_buf_alloc(len: usize) -> Result<*mut u8, VoodooBufferError> {
    let placeholder1 = VirtualAlloc2(
        0,
        ptr::null(),
        2 * len,
        MEM_RESERVE | MEM_RESERVE_PLACEHOLDER,
        PAGE_NOACCESS,
        ptr::null_mut(),
        0,
    );

    if placeholder1.is_null() {
        return Err(VoodooBufferError::OOM);
    }

    if VirtualFree(placeholder1, len, MEM_RELEASE | MEM_PRESERVE_PLACEHOLDER) == FALSE {
        return Err(VoodooBufferError::OOM);
    }

    let handle = CreateFileMappingA(
        INVALID_HANDLE_VALUE,
        ptr::null(),
        PAGE_READWRITE,
        0,
        len as u32,
        ptr::null(),
    );

    if handle == 0 {
        VirtualFree(placeholder1, 0, MEM_RELEASE);
        return Err(VoodooBufferError::OOM);
    }

    let view1 = MapViewOfFile3(
        handle,
        0,
        placeholder1,
        0,
        len,
        MEM_REPLACE_PLACEHOLDER,
        PAGE_READWRITE,
        ptr::null_mut(),
        0,
    );

    if view1 == 0 {
        VirtualFree(placeholder1, 0, MEM_RELEASE);
        return Err(VoodooBufferError::OOM);
    }

    let placeholder2 = placeholder1.add(len);
    let view2 = MapViewOfFile3(
        handle,
        0,
        placeholder2,
        0,
        len,
        MEM_REPLACE_PLACEHOLDER,
        PAGE_READWRITE,
        ptr::null_mut(),
        0,
    );

    assert_ne!(0, view2);
    CloseHandle(handle);

    Ok(view1 as *mut _)
}

pub(super) unsafe fn voodoo_buf_free(addr: *mut u8, len: usize) {
    UnmapViewOfFile(addr.add(len) as _);
    UnmapViewOfFile(addr as _);
}
