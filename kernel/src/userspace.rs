use crate::println;
use core::arch::asm;
use x86_64::{
    PrivilegeLevel, VirtAddr,
    registers::segmentation::{CS, Segment},
    structures::{
        gdt::SegmentSelector,
        paging::{PageTable, PageTableFlags, PhysFrame},
    },
};

use crate::memory;

unsafe fn prepare_paging(physical_memory_offset: VirtAddr) {
    // FIXME: HADOUKEN
    unsafe {
        for entry in memory::active_level_4_table(physical_memory_offset).iter_mut() {
            if entry.is_unused() {
                continue;
            }

            entry.set_flags(entry.flags() | PageTableFlags::USER_ACCESSIBLE);

            if let Ok(level_3_page_table) = entry.frame().map(|frame: PhysFrame| {
                &mut *(physical_memory_offset + frame.start_address().as_u64())
                    .as_mut_ptr::<PageTable>()
            }) {
                for entry in level_3_page_table.iter_mut() {
                    if entry.is_unused() {
                        continue;
                    }

                    entry.set_flags(entry.flags() | PageTableFlags::USER_ACCESSIBLE);

                    if let Ok(level_2_page_table) = entry.frame().map(|frame| {
                        &mut *(physical_memory_offset + frame.start_address().as_u64())
                            .as_mut_ptr::<PageTable>()
                    }) {
                        for entry in level_2_page_table.iter_mut() {
                            if entry.is_unused() {
                                continue;
                            }

                            entry.set_flags(entry.flags() | PageTableFlags::USER_ACCESSIBLE);

                            if let Ok(level_1_page_table) = entry.frame().map(|frame| {
                                &mut *(physical_memory_offset + frame.start_address().as_u64())
                                    .as_mut_ptr::<PageTable>()
                            }) {
                                for entry in level_1_page_table.iter_mut() {
                                    if entry.is_unused() {
                                        continue;
                                    }

                                    entry
                                        .set_flags(entry.flags() | PageTableFlags::USER_ACCESSIBLE);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub unsafe fn jump_to_userspace(physical_memory_offset: VirtAddr) {
    unsafe {
        prepare_paging(physical_memory_offset);

        asm!(
            "mov ax, dx",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov {tmp:r}, rsp",
            "push rdx", // SS (DS)
            "push {tmp:r}", // Current ESP
            "pushfq", // EFLAGS
            "push {code_selector:r}", // CS
            "push {user_code:r}", // EIP
            "iretq",
            user_code = in(reg) user_code as usize,
            tmp = out(reg) _,
            in("rdx") SegmentSelector::new(4, PrivilegeLevel::Ring3).0,
            code_selector = in(reg) SegmentSelector::new(3, PrivilegeLevel::Ring3).0,
        );
    }
}

fn current_ring() -> u16 {
    return CS::get_reg().0 & 0b11;
}

pub fn is_user_ring() -> bool {
    return current_ring() == 3;
}

pub fn user_code() {
    println!(
        "Estamos executando codigo de usuario? Ring {:#?}",
        current_ring()
    );
    loop {}
}
