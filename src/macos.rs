// This implementation is based on
// https://github.com/gnzlbg/slice_deque/blob/master/src/mirrored/macos.rs

use crate::VoodooBufferError;

use mach2::{
    boolean::boolean_t,
    kern_return::KERN_SUCCESS,
    mach_types::mem_entry_name_port_t,
    memory_object_types::memory_object_size_t,
    traps::mach_task_self,
    vm::{mach_make_memory_entry_64, mach_vm_allocate, mach_vm_deallocate, mach_vm_remap},
    vm_inherit::VM_INHERIT_NONE,
    vm_page_size::vm_page_size,
    vm_prot::{vm_prot_t, VM_PROT_READ, VM_PROT_WRITE},
    vm_statistics::{VM_FLAGS_ANYWHERE, VM_FLAGS_FIXED, VM_FLAGS_OVERWRITE},
    vm_types::mach_vm_address_t,
};

use std::mem::MaybeUninit;

pub(super) unsafe fn voodoo_buf_min_len() -> usize {
    vm_page_size
}

pub(super) unsafe fn voodoo_buf_alloc(len: usize) -> Result<*mut u8, VoodooBufferError> {
    let task = mach_task_self();

    let mut addr: mach_vm_address_t = 0;
    let result = mach_vm_allocate(task, &mut addr as _, (len * 2) as u64, VM_FLAGS_ANYWHERE);

    if result != KERN_SUCCESS {
        return Err(VoodooBufferError::OOM);
    }

    let result = mach_vm_allocate(
        task,
        &mut addr as _,
        len as u64,
        VM_FLAGS_FIXED | VM_FLAGS_OVERWRITE,
    );

    if result != KERN_SUCCESS {
        return Err(VoodooBufferError::OOM);
    }

    let mut memory_object_size = len as memory_object_size_t;
    let mut object_handle = MaybeUninit::<mem_entry_name_port_t>::uninit();
    let result = mach_make_memory_entry_64(
        task,
        &mut memory_object_size as _,
        addr as _,
        VM_PROT_READ | VM_PROT_WRITE,
        object_handle.as_mut_ptr(),
        0,
    );

    if result != KERN_SUCCESS {
        let result = mach_vm_deallocate(task, addr, (len * 2) as u64);
        assert_eq!(result, KERN_SUCCESS);
        return Err(VoodooBufferError::OOM);
    }

    let mut to = (addr as *mut u8).add(len) as mach_vm_address_t;
    let mut current_prot = MaybeUninit::<vm_prot_t>::uninit();
    let mut out_prot = MaybeUninit::<vm_prot_t>::uninit();
    let result = mach_vm_remap(
        task,
        &mut to as _,
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
        return Err(VoodooBufferError::OOM);
    }

    Ok(addr as _)
}

pub(super) unsafe fn voodoo_buf_free(addr: *mut u8, len: usize) {
    let result = mach_vm_deallocate(mach_task_self(), addr as _, (len * 2) as u64);
    assert_eq!(result, KERN_SUCCESS, "de-allocation failed");
}
