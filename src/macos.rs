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
use std::mem::MaybeUninit;

pub(super) unsafe fn voodoo_buf_min_len() -> usize {
    vm_page_size
}

pub(super) unsafe fn voodoo_buf_alloc(len: usize) -> Result<*mut u8, BufferError> {
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

    Ok(addr as _)
}

pub(super) unsafe fn voodoo_buf_free(addr: *mut u8, len: usize) {
    let result = mach_vm_deallocate(mach_task_self(), addr as _, (len * 2) as u64);
    assert_eq!(result, KERN_SUCCESS, "de-allocation failed");
}
