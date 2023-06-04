use crate::BufferError;
use mach2::boolean::boolean_t;
use mach2::kern_return::KERN_SUCCESS;
use mach2::mach_types::mem_entry_name_port_t;
use mach2::memory_object_types::{memory_object_offset_t, memory_object_size_t};
use mach2::traps::mach_task_self;
use mach2::vm::{mach_make_memory_entry_64, mach_vm_allocate, mach_vm_deallocate, mach_vm_remap};
use mach2::vm_inherit::VM_INHERIT_NONE;
use mach2::vm_page_size::vm_page_size;
use mach2::vm_prot::{vm_prot_t, VM_PROT_READ, VM_PROT_WRITE};
use mach2::vm_statistics::{VM_FLAGS_ANYWHERE, VM_FLAGS_FIXED, VM_FLAGS_OVERWRITE};
use mach2::vm_types::mach_vm_address_t;
use std::{
    mem::MaybeUninit,
    ptr::{slice_from_raw_parts, slice_from_raw_parts_mut},
};

#[derive(Debug)]
pub struct VoodooBuffer {
    addr: *mut u8,
    len: usize,
}

impl VoodooBuffer {
    pub fn new(len: usize) -> Result<Self, BufferError> {
        if !len.is_power_of_two() {
            return Err(BufferError {
                msg: "len must be power of two".to_string(),
            });
        }

        println!("{} is power of 2", len);

        unsafe {
            if len % vm_page_size != 0 {
                return Err(BufferError {
                    msg: format!("len must be page aligned, {}", vm_page_size),
                });
            }

            let task = mach_task_self();

            let mut addr: mach_vm_address_t = 0;
            let result = mach_vm_allocate(
                task,
                &mut addr as *mut mach_vm_address_t,
                (len * 2) as u64,
                VM_FLAGS_ANYWHERE,
            );

            if result != KERN_SUCCESS {
                return Err(BufferError {
                    msg: "out of memory".to_string(),
                });
            }

            let result = mach_vm_allocate(
                task,
                &mut addr as *mut mach_vm_address_t,
                len as u64,
                VM_FLAGS_FIXED | VM_FLAGS_OVERWRITE,
            );

            if result != KERN_SUCCESS {
                return Err(BufferError {
                    msg: "re-allocation failed".to_string(),
                });
            }

            let mut memory_object_size = len as memory_object_size_t;
            let mut object_handle = MaybeUninit::<mem_entry_name_port_t>::uninit();
            let result = mach_make_memory_entry_64(
                task,
                &mut memory_object_size as *mut memory_object_size_t,
                addr as memory_object_offset_t,
                VM_PROT_READ | VM_PROT_WRITE,
                object_handle.as_mut_ptr(),
                0,
            );

            if result != KERN_SUCCESS {
                let result = mach_vm_deallocate(task, addr, (len * 2) as u64);
                assert_eq!(result, KERN_SUCCESS);
                return Err(BufferError {
                    msg: "re-allocation failed".to_string(),
                });
            }

            let mut to = (addr as *mut u8).add(len) as mach_vm_address_t;
            let mut current_prot = MaybeUninit::<vm_prot_t>::uninit();
            let mut out_prot = MaybeUninit::<vm_prot_t>::uninit();
            let result = mach_vm_remap(
                task,
                &mut to as *mut mach_vm_address_t,
                len as u64,
                0,
                VM_FLAGS_FIXED | VM_FLAGS_OVERWRITE,
                task,
                addr,
                0 as boolean_t,
                current_prot.as_mut_ptr(),
                out_prot.as_mut_ptr(),
                VM_INHERIT_NONE,
            );

            if result != KERN_SUCCESS {
                let result = mach_vm_deallocate(task, addr, (len * 2) as u64);
                assert_eq!(result, KERN_SUCCESS);
                return Err(BufferError {
                    msg: "re-allocation failed".to_string(),
                });
            }

            Ok(Self {
                addr: addr as _,
                len,
            })
        }
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

impl Drop for VoodooBuffer {
    fn drop(&mut self) {
        unsafe {
            let result =
                mach_vm_deallocate(mach_task_self(), self.addr as _, (self.len * 2) as u64);
            assert_eq!(result, KERN_SUCCESS);
        }
    }
}
