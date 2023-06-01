use crate::BufferError;

use std::mem::MaybeUninit;
use std::ops::{
    Deref, DerefMut, Index, IndexMut, Range, RangeFrom, RangeFull, RangeTo, RangeToInclusive,
};
use std::ptr::{self, slice_from_raw_parts, slice_from_raw_parts_mut};
use windows_sys::core::PWSTR;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, FALSE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Diagnostics::Debug::{
    FormatMessageW, FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
    FORMAT_MESSAGE_IGNORE_INSERTS,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingA, LocalFree, MapViewOfFile3, UnmapViewOfFile, VirtualAlloc2, VirtualFree,
    MEM_PRESERVE_PLACEHOLDER, MEM_RELEASE, MEM_REPLACE_PLACEHOLDER, MEM_RESERVE,
    MEM_RESERVE_PLACEHOLDER, PAGE_NOACCESS, PAGE_READWRITE,
};
use windows_sys::Win32::System::SystemInformation;
use windows_sys::Win32::System::SystemInformation::SYSTEM_INFO;

#[derive(Debug)]
pub struct InfiniteBuffer {
    handle: HANDLE,
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
                msg: format!("len must be power of two"),
            });
        }

        let sys_info = unsafe {
            let mut sys_info = MaybeUninit::<SYSTEM_INFO>::zeroed();
            SystemInformation::GetSystemInfo(sys_info.as_mut_ptr());
            sys_info.assume_init()
        };

        if (sys_info.dwAllocationGranularity as usize) % len != 0 {
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
            handle,
            len,
        })
    }
}

impl Drop for InfiniteBuffer {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.addr.add(self.len) as _);
            UnmapViewOfFile(self.addr as _);
            CloseHandle(self.handle);
        }
    }
}

impl Deref for InfiniteBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { &*(slice_from_raw_parts(self.addr, self.len)) }
    }
}

impl DerefMut for InfiniteBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *slice_from_raw_parts_mut(self.addr, self.len) }
    }
}

impl Index<usize> for InfiniteBuffer {
    type Output = u8;

    fn index(&self, mut index: usize) -> &Self::Output {
        index = index % self.len;
        unsafe { &*(self.addr.add(index)) }
    }
}

impl IndexMut<usize> for InfiniteBuffer {
    fn index_mut(&mut self, mut index: usize) -> &mut Self::Output {
        index = index % self.len;
        unsafe { &mut *(self.addr.add(index)) }
    }
}

impl Index<Range<usize>> for InfiniteBuffer {
    type Output = [u8];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        if index.start > index.end {
            return &[];
        }

        let len = index.end - index.start;
        if len > self.len {
            panic!("out of bounds")
        }

        let index = index.start % self.len;
        unsafe { &*(slice_from_raw_parts(self.addr.add(index), len)) }
    }
}

impl IndexMut<Range<usize>> for InfiniteBuffer {
    fn index_mut(&mut self, index: Range<usize>) -> &mut Self::Output {
        if index.start > index.end {
            return &mut [];
        }

        let len = index.end - index.start;
        if len > self.len {
            panic!("out of bounds")
        }

        let index = index.start % self.len;
        unsafe { &mut *(slice_from_raw_parts_mut(self.addr.add(index), len)) }
    }
}

impl Index<RangeTo<usize>> for InfiniteBuffer {
    type Output = [u8];

    fn index(&self, index: RangeTo<usize>) -> &Self::Output {
        let start = index.end - self.len;
        let index = start % self.len;
        unsafe { &*(slice_from_raw_parts(self.addr.add(index), self.len)) }
    }
}

impl IndexMut<RangeTo<usize>> for InfiniteBuffer {
    fn index_mut(&mut self, index: RangeTo<usize>) -> &mut Self::Output {
        let start = index.end - self.len;
        let index = start % self.len;
        unsafe { &mut *(slice_from_raw_parts_mut(self.addr.add(index), self.len)) }
    }
}

impl Index<RangeFrom<usize>> for InfiniteBuffer {
    type Output = [u8];

    fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
        let index = index.start % self.len;
        unsafe { &*(slice_from_raw_parts(self.addr.add(index), self.len)) }
    }
}

impl IndexMut<RangeFrom<usize>> for InfiniteBuffer {
    fn index_mut(&mut self, index: RangeFrom<usize>) -> &mut Self::Output {
        let index = index.start % self.len;
        unsafe { &mut *(slice_from_raw_parts_mut(self.addr.add(index), self.len)) }
    }
}

impl Index<RangeToInclusive<usize>> for InfiniteBuffer {
    type Output = [u8];

    fn index(&self, index: RangeToInclusive<usize>) -> &Self::Output {
        let start = index.end - self.len + 1;
        let index = start % self.len;
        unsafe { &*(slice_from_raw_parts(self.addr.add(index), self.len)) }
    }
}

impl IndexMut<RangeToInclusive<usize>> for InfiniteBuffer {
    fn index_mut(&mut self, index: RangeToInclusive<usize>) -> &mut Self::Output {
        let start = index.end - self.len + 1;
        let index = start % self.len;
        unsafe { &mut *(slice_from_raw_parts_mut(self.addr.add(index), self.len)) }
    }
}

impl Index<RangeFull> for InfiniteBuffer {
    type Output = [u8];

    fn index(&self, _: RangeFull) -> &Self::Output {
        unsafe { &*(slice_from_raw_parts(self.addr, self.len)) }
    }
}

impl IndexMut<RangeFull> for InfiniteBuffer {
    fn index_mut(&mut self, _: RangeFull) -> &mut Self::Output {
        unsafe { &mut *(slice_from_raw_parts_mut(self.addr, self.len)) }
    }
}
