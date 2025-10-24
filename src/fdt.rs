use spin::Once;
use fdtree_rs::{LinuxFdt, InterruptController};
use log::*;
use axplat::mem::VirtAddr;

pub static FDT: Once<LinuxFdt> = Once::new();

pub(crate) fn init_fdt(fdt_paddr: VirtAddr) {
    info!("FDT addr is: {:x}", fdt_paddr.as_usize());
    let fdt = unsafe {
        LinuxFdt::from_ptr(fdt_paddr.as_usize() as  *const u8).expect("Failed to parse FDT")
    };
    FDT.call_once(|| fdt);
}

pub(crate) fn interrupt_controller() -> Option<InterruptController<'static, 'static>> {
    let fdt = FDT.get().expect("FDT is not initialized");
    match fdt.interrupt_controller() {
        Some(ic_node) => Some(ic_node),
        None => {
            warn!("No interrupt-controller node found in FDT");
            None
        }
    }
}
