use spin::Once;
use fdtree_rs::LinuxFdt;
use log::*;
use axplat::mem::VirtAddr;

pub static FDT: Once<LinuxFdt> = Once::new();

pub(crate) fn init_fdt(fdt_paddr: VirtAddr) {
    info!("FDT PA{:x}", fdt_paddr.as_usize());
    let fdt = unsafe {
        LinuxFdt::from_ptr(fdt_paddr.as_usize() as  *const u8).expect("Failed to parse FDT")
    };
    debug!("fdt {:?}", fdt);
    FDT.call_once(|| fdt);
}

