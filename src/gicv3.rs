//! ARM Generic Interrupt Controller (GIC).
use arm_gic::gicv3::GicV3;
use log::*;

use axplat::mem::VirtAddr;
use aarch64_cpu::registers::*;

/// Initializes GIC
pub fn init_gic(gicd_base: VirtAddr, gicr_base: VirtAddr) {
    info!("Initialize GICv3... from 0x{:x} 0x{:x}", gicd_base.as_usize(), gicr_base.as_usize());
    const GICR_RD_OFFSET: usize = 0x20000;
    const GICR_TYPER_HI_OFFSET: usize = 0x0008;
    let mpidr_aff: u64 = aarch64_cpu::registers::MPIDR_EL1.get() & 0xffffff;
    let mut cur_gicr_base: usize = gicr_base.as_usize();
    loop {
        let gicr_typer_aff: u64 = unsafe {
              core::ptr::read_volatile((cur_gicr_base + GICR_TYPER_HI_OFFSET) as *const u64)
        };
        trace!("gicr_typer_aff: 0x{:x?}", gicr_typer_aff);
        if mpidr_aff == gicr_typer_aff >> 32 {
            trace!("cur_gicr_base: 0x{:x}", cur_gicr_base);
            break;
        }
        cur_gicr_base += GICR_RD_OFFSET;
    }

    let mut v3: GicV3 = unsafe { GicV3::new(gicd_base.as_mut_ptr_of(), cur_gicr_base as *mut u64) };
    v3.setup();
}

