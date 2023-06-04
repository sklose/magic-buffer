use crate::BufferError;

use std::cmp::max;
use std::{mem::MaybeUninit, ptr};

use windows_sys::core::PWSTR;
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, FALSE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::Debug::{
            FormatMessageW, FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
            FORMAT_MESSAGE_IGNORE_INSERTS,
        },
        Memory::{
            CreateFileMappingA, LocalFree, MapViewOfFile3, UnmapViewOfFile, VirtualAlloc2,
            VirtualFree, MEM_PRESERVE_PLACEHOLDER, MEM_RELEASE, MEM_REPLACE_PLACEHOLDER,
            MEM_RESERVE, MEM_RESERVE_PLACEHOLDER, PAGE_NOACCESS, PAGE_READWRITE,
        },
        SystemInformation::{self, SYSTEM_INFO},
    },
};

fn last_error_message() -> String {
    unsafe {
        let code = GetLastError();
        let mut lp_buffer: PWSTR = ptr::null_mut();
        let cb_buffer = FormatMessageW(
            FORMAT_MESSAGE_ALLOCATE_BUFFER
                | FORMAT_MESSAGE_FROM_SYSTEM
                | FORMAT_MESSAGE_IGNORE_INSERTS,
            ptr::null(),
            code,
            0,
            (&mut lp_buffer as *mut _) as _,
            0,
            ptr::null(),
        );

        if cb_buffer == 0 {
            return code.to_string();
        }

        let buffer = std::slice::from_raw_parts(lp_buffer, cb_buffer as usize - 1);
        LocalFree(lp_buffer as _);

        String::from_utf16_lossy(buffer)
    }
}

pub(super) unsafe fn voodoo_buf_min_len() -> usize {
    let mut sys_info = MaybeUninit::<SYSTEM_INFO>::zeroed();
    SystemInformation::GetSystemInfo(sys_info.as_mut_ptr());
    let sys_info = sys_info.assume_init();

    max(sys_info.dwPageSize, sys_info.dwAllocationGranularity) as usize
}

pub(super) unsafe fn voodoo_buf_alloc(len: usize) -> Result<*mut u8, BufferError> {
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
        return Err(BufferError {
            msg: last_error_message(),
        });
    }

    if VirtualFree(placeholder1, len, MEM_RELEASE | MEM_PRESERVE_PLACEHOLDER) == FALSE {
        return Err(BufferError {
            msg: last_error_message(),
        });
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
        return Err(BufferError {
            msg: last_error_message(),
        });
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
        return Err(BufferError {
            msg: last_error_message(),
        });
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

    if view2 == 0 {
        panic!("failed")
    }

    CloseHandle(handle);

    Ok(view1 as *mut _)
}

pub(super) unsafe fn voodoo_buf_free(addr: *mut u8, len: usize) {
    UnmapViewOfFile(addr.add(len) as _);
    UnmapViewOfFile(addr as _);
}
