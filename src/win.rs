use crate::BufferError;

use std::{
    mem::MaybeUninit,
    ptr::{self, slice_from_raw_parts, slice_from_raw_parts_mut},
};

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

#[derive(Debug)]
pub struct InfiniteBuffer {
    addr: *mut u8,
    len: usize,
}

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

        if cb_buffer <= 0 {
            return code.to_string();
        }

        let buffer = std::slice::from_raw_parts(lp_buffer, cb_buffer as usize - 1);
        LocalFree(lp_buffer as _);

        String::from_utf16_lossy(buffer)
    }
}

impl InfiniteBuffer {
    pub fn new(len: usize) -> Result<Self, BufferError> {
        if !len.is_power_of_two() {
            return Err(BufferError {
                msg: "len must be power of two".to_string(),
            });
        }

        let sys_info = unsafe {
            let mut sys_info = MaybeUninit::<SYSTEM_INFO>::zeroed();
            SystemInformation::GetSystemInfo(sys_info.as_mut_ptr());
            sys_info.assume_init()
        };

        if len % (sys_info.dwAllocationGranularity as usize) != 0 {
            return Err(BufferError {
                msg: format!(
                    "len must be page aligned, {}",
                    sys_info.dwAllocationGranularity
                ),
            });
        }

        let placeholder1 = unsafe {
            VirtualAlloc2(
                0,
                ptr::null(),
                2 * len,
                MEM_RESERVE | MEM_RESERVE_PLACEHOLDER,
                PAGE_NOACCESS,
                ptr::null_mut(),
                0,
            )
        };

        if placeholder1.is_null() {
            return Err(BufferError {
                msg: last_error_message(),
            });
        }

        unsafe {
            if VirtualFree(placeholder1, len, MEM_RELEASE | MEM_PRESERVE_PLACEHOLDER) == FALSE {
                return Err(BufferError {
                    msg: last_error_message(),
                });
            }
        };

        let handle = unsafe {
            CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                ptr::null(),
                PAGE_READWRITE,
                0,
                len as u32,
                ptr::null(),
            )
        };

        if handle == 0 {
            unsafe {
                VirtualFree(placeholder1, 0, MEM_RELEASE);
            }
            return Err(BufferError {
                msg: last_error_message(),
            });
        }

        let view1 = unsafe {
            MapViewOfFile3(
                handle,
                0,
                placeholder1,
                0,
                len,
                MEM_REPLACE_PLACEHOLDER,
                PAGE_READWRITE,
                ptr::null_mut(),
                0,
            )
        };

        if view1 == 0 {
            unsafe {
                VirtualFree(placeholder1, 0, MEM_RELEASE);
            }
            return Err(BufferError {
                msg: last_error_message(),
            });
        }

        let placeholder2 = unsafe { placeholder1.add(len) };
        let view2 = unsafe {
            MapViewOfFile3(
                handle,
                0,
                placeholder2,
                0,
                len,
                MEM_REPLACE_PLACEHOLDER,
                PAGE_READWRITE,
                ptr::null_mut(),
                0,
            )
        };

        if view2 == 0 {
            panic!("failed")
        }

        unsafe {
            CloseHandle(handle);
        }

        Ok(Self {
            addr: view1 as *mut _,
            len,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub(crate) unsafe fn as_slice(&self, offset: usize, len: usize) -> &[u8] {
        &*(slice_from_raw_parts(self.addr.add(offset), len))
    }

    #[inline(always)]
    pub(crate) unsafe fn as_slice_mut(&mut self, offset: usize, len: usize) -> &mut [u8] {
        &mut *(slice_from_raw_parts_mut(self.addr.add(offset), len))
    }
}

impl Drop for InfiniteBuffer {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.addr.add(self.len) as _);
            UnmapViewOfFile(self.addr as _);
        }
    }
}
