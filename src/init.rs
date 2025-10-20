use axplat::init::InitIf;

#[allow(unused_imports)]
use crate::config::devices::{GICR_PADDR, GICD_PADDR, TIMER_IRQ, UART_IRQ, UART_PADDR};
use crate::config::plat::PSCI_METHOD;
use axplat::mem::{pa, phys_to_virt};

struct InitIfImpl;

use fdtree_rs::LinuxFdt;

#[unsafe(no_mangle)]
pub extern "C" fn _boot_print_usize(num: usize) {
    let mut msg: [u8; 16] = [0; 16];
    let mut num = num;
    let mut cnt = 0;

    boot_print_str("0x");
    if num == 0 {
        boot_serial_send('0' as u8);
    } else {
        loop {
            if num == 0 {
                break;
            }
            msg[cnt] = match (num & 0xf) as u8 {
                n if n < 10 => n + '0' as u8,
                n => n - 10 + 'a' as u8,
            };
            cnt += 1;
            num >>= 4;
        }
        for i in 0..cnt {
            boot_serial_send(msg[cnt - i - 1]);
        }
    }
    boot_print_str("\r\n");
}

/// 打印字符串
pub fn boot_print_str(data: &str) {
    for byte in data.bytes() {
        boot_serial_send(byte);
    }
}

#[derive(Copy, Clone, Debug)]
/// Struct representing a NS16550A UART peripheral
pub struct Uart {
	/// Base address of the peripheral
	base_address: usize,
}

impl Uart {
	/// Creates a new instance of `Uart` with the given base address.
	pub const fn new(base_address: usize) -> Self {
		Self { base_address }
	}

	/// If the transmitter holding register is empty, writes `c` in the transmitter holding register, and returns `c`. Otherwise returns `None`.
	pub fn put(&self, c: u8) -> Option<u8> {
		let ptr = self.base_address as *mut u8;
		unsafe {
			ptr.write_volatile(c);
		}
		Some(c)
	}
}

static BOOT_SERIAL: Uart = Uart::new(0x3f8);

/// 打印字节
#[allow(unused)]
pub fn boot_serial_send(data: u8) {
    unsafe { BOOT_SERIAL.put(data) };
}

#[impl_plat_interface]
impl InitIf for InitIfImpl {
    /// Initializes the platform at the early stage for the primary core.
    ///
    /// This function should be called immediately after the kernel has booted,
    /// and performed earliest platform configuration and initialization (e.g.,
    /// early console, clocking).
    fn init_early(_cpu_id: usize, _dtb: usize) {
        axcpu::init::init_trap();
        axplat_aarch64_peripherals::ns16550a::init_early(phys_to_virt(pa!(UART_PADDR)));
        axplat_aarch64_peripherals::psci::init(PSCI_METHOD);
        axplat_aarch64_peripherals::generic_timer::init_early();
        //#[cfg(feature = "rtc")]
        //axplat_aarch64_peripherals::pl031::init_early(phys_to_virt(pa!(RTC_PADDR)));
    }

    /// Initializes the platform at the early stage for secondary cores.
    #[cfg(feature = "smp")]
    fn init_early_secondary(_cpu_id: usize) {
        axcpu::init::init_trap();
    }

    /// Initializes the platform at the later stage for the primary core.
    ///
    /// This function should be called after the kernel has done part of its
    /// initialization (e.g, logging, memory management), and finalized the rest of
    /// platform configuration and initialization.
    fn init_later(_cpu_id: usize, dtb: usize) {
        #[cfg(feature = "irq")]
        {
            use log::info;
            info!("FDT {:x}", dtb);
            info!("cpu_id {}",_cpu_id);
            unsafe {
                let fdt = LinuxFdt::from_ptr(phys_to_virt(dtb.into()).as_usize() as *const u8).unwrap();
                info!("fdt {:?}", fdt);
            }
            axplat_aarch64_peripherals::gic::init_gic(
                phys_to_virt(pa!(GICD_PADDR)),
                phys_to_virt(pa!(GICR_PADDR)),
            );
            axplat_aarch64_peripherals::gic::init_gicr();
            axplat_aarch64_peripherals::generic_timer::enable_irqs(TIMER_IRQ);
        }
    }

    /// Initializes the platform at the later stage for secondary cores.
    #[cfg(feature = "smp")]
    fn init_later_secondary(_cpu_id: usize) {
        #[cfg(feature = "irq")]
        {
            axplat_aarch64_peripherals::gic::init_gicr();
            axplat_aarch64_peripherals::generic_timer::enable_irqs(TIMER_IRQ);
        }
    }
}
