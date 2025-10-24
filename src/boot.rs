use axplat::mem::{Aligned4K, pa};
use page_table_entry::{GenericPTE, MappingFlags, aarch64::A64PTE};
use aarch64_cpu::registers::*;

use crate::config::plat::{BOOT_STACK_SIZE, PHYS_VIRT_OFFSET};

#[unsafe(link_section = ".bss.stack")]
static mut BOOT_STACK: [u8; BOOT_STACK_SIZE] = [0; BOOT_STACK_SIZE];

#[unsafe(link_section = ".data")]
static mut BOOT_PT_L0: Aligned4K<[A64PTE; 512]> = Aligned4K::new([A64PTE::empty(); 512]);

#[unsafe(link_section = ".data")]
static mut BOOT_PT_L1: Aligned4K<[A64PTE; 512]> = Aligned4K::new([A64PTE::empty(); 512]);

use crate::serial::{boot_print_str, boot_print_usize};

unsafe fn init_boot_page_table() {
    boot_print_str("[boot] init boot page table\r\n");
    crate::psci::kvm_guard_granule_init();

    boot_print_str("[boot] kvm xmap pci cam\r\n");
    crate::psci::do_xmap_granules(0x7200_0000, 0x100_0000);

    boot_print_str("[boot] kvm xmap pci mem\r\n");
    crate::psci::do_xmap_granules(0x7000_0000, 0x200_0000);

    boot_print_str("[boot] kvm xmap gicv3 mem\r\n");
    crate::psci::do_xmap_granules(0x3ffb_0000, 0x20_0000);

    unsafe {
        // 0x0000_0000_0000 ~ 0x0080_0000_0000, table
        BOOT_PT_L0[0] = A64PTE::new_table(pa!(&raw mut BOOT_PT_L1 as usize));
        // 0x0000_0000_0000..0x0000_4000_0000, 1G block, device memory
        BOOT_PT_L1[0] = A64PTE::new_page(
            pa!(0),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
            true,
        );
        // 0x0000_4000_0000..0x0000_8000_0000, 1G block, normal memory
        BOOT_PT_L1[1] = A64PTE::new_page(
            pa!(0x4000_0000),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
            true,
        );

        // 0x0000_8000_0000..0x0000_C000_0000, 1G block, normal memory_set
        BOOT_PT_L1[2] = A64PTE::new_page(
            pa!(0x8000_0000),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
            true,
        );

        // 0x0000_C000_0000..0x0001_0000_0000, 1G block, normal memory_set
        BOOT_PT_L1[3] = A64PTE::new_page(
            pa!(0xC000_0000),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
            true,
        );
    }

}

#[unsafe(no_mangle)]
extern "C" fn kernel_main_test() {
    boot_print_str("[boot] kernel main entered cpu id\r\n");
}

unsafe fn enable_fp() {
    // FP/SIMD needs to be enabled early, as the compiler may generate SIMD
    // instructions in the bootstrapping code to speed up the operations
    // like `memset` and `memcpy`.
    #[cfg(feature = "fp-simd")]
    axcpu::asm::enable_fp();
}

/// Kernel entry point with Linux image header.
///
/// Some bootloaders require this header to be present at the beginning of the
/// kernel image.
///
/// Documentation: <https://docs.kernel.org/arch/arm64/booting.html>
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!("
         bl       {entry}             // Branch to kernel start, magic
        .space 52, 0
        .inst 0x644d5241
        .space 4, 0
    ",
    entry = sym _start_primary,
    )
}

// 根据需要切换至EL1
unsafe fn switch_to_el1() {
    let current_sp = aarch64_cpu::registers::SP.get();
    // 各异常级别使用不同SP
    SPSel.write(SPSel::SP::ELx);
    aarch64_cpu::registers::SP.set(current_sp);
    // 根据需要切换至EL1
    let current_el = CurrentEL.read(CurrentEL::EL);
    boot_print_str("[boot] Current el ");
    boot_print_usize(current_el as _);
}

/// The earliest entry point for the primary CPU.
#[unsafe(naked)]
#[unsafe(link_section = ".text.boot")]
unsafe extern "C" fn _start_primary() -> ! {
    // X0 = dtb
    core::arch::naked_asm!("
        mrs     x19, mpidr_el1
        and     x19, x19, #0xffffff     // get current CPU id
        mov     x20, x0                 // save DTB pointer

        adrp    x8, {boot_stack}        // setup boot stack
        add     x8, x8, {boot_stack_size}
        mov     sp, x8

        bl      {switch_to_el1}         // switch to EL1
        bl      {enable_fp}             // enable fp/neon

        bl      {init_boot_page_table}
        adrp    x0, {boot_pt}
        bl      {init_mmu}              // setup MMU

        mov     x8, {phys_virt_offset}  // set SP to the high address
        add     sp, sp, x8

        mov     x0, x19                 // call_main(cpu_id, dtb)
        mov     x1, x20
        ldr     x8, ={entry}
        blr     x8
        b .
        ",
        switch_to_el1 = sym switch_to_el1,
        init_boot_page_table = sym init_boot_page_table,
        init_mmu = sym axcpu::init::init_mmu,
        enable_fp = sym enable_fp,
        boot_pt = sym BOOT_PT_L0,
        phys_virt_offset = const PHYS_VIRT_OFFSET,
        entry = sym axplat::call_main,
        boot_stack = sym BOOT_STACK,
        boot_stack_size = const BOOT_STACK_SIZE,
    )
}

/// The earliest entry point for the secondary CPUs.
#[cfg(feature = "smp")]
#[unsafe(naked)]
#[unsafe(link_section = ".text.boot")]
pub(crate) unsafe extern "C" fn _start_secondary() -> ! {
    // X0 = stack pointer
    core::arch::naked_asm!("
        mrs     x19, mpidr_el1
        and     x19, x19, #0xffffff     // get current CPU id

        mov     sp, x0
        bl      {switch_to_el1}
        bl      {enable_fp}
        adrp    x0, {boot_pt}
        bl      {init_mmu}

        mov     x8, {phys_virt_offset}  // set SP to the high address
        add     sp, sp, x8

        mov     x0, x19                 // call_secondary_main(cpu_id)
        ldr     x8, ={entry}
        blr     x8
        b      .",
        switch_to_el1 = sym axcpu::init::switch_to_el1,
        init_mmu = sym axcpu::init::init_mmu,
        enable_fp = sym enable_fp,
        boot_pt = sym BOOT_PT_L0,
        phys_virt_offset = const PHYS_VIRT_OFFSET,
        entry = sym axplat::call_secondary_main,
    )
}
