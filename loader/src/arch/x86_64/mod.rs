// Copyright (c) 2018 Colin Finck, RWTH Aachen University
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

pub mod paging;
pub mod processor;
pub mod serial;

use arch::x86_64::paging::{BasePageSize, LargePageSize, PageSize, PageTableEntryFlags};
use arch::x86_64::serial::SerialPort;
use elf::*;
use hermit_multiboot::Multiboot;
use physicalmem;

extern "C" {
	static mb_info: usize;
}

// CONSTANTS
pub const ELF_ARCH: u16 = ELF_EM_X86_64;

const HERMIT_KERNEL_OFFSET_BASE:       usize = 0x08;
const HERMIT_KERNEL_OFFSET_IMAGE_SIZE: usize = 0x38;
const HERMIT_KERNEL_OFFSET_UARTPORT:   usize = 0x98;
const HERMIT_KERNEL_OFFSET_CMDLINE:    usize = 0xA0;
const HERMIT_KERNEL_OFFSET_CMDSIZE:    usize = 0xA8;

const SERIAL_PORT_ADDRESS: u16 = 0x3F8;
const SERIAL_PORT_BAUDRATE: u32 = 115200;

// VARIABLES
static COM1: SerialPort = SerialPort::new(SERIAL_PORT_ADDRESS);

lazy_static! {
	static ref MULTIBOOT: Multiboot = unsafe { Multiboot::new(mb_info) };
}

// FUNCTIONS
pub fn message_output_init() {
	COM1.init(SERIAL_PORT_BAUDRATE);
}

pub fn output_message_byte(byte: u8) {
	COM1.write_byte(byte);
}

pub unsafe fn find_kernel() -> usize {
	// Identity-map the Multiboot information.
	assert!(mb_info > 0, "Could not find Multiboot information");
	loaderlog!("Found Multiboot information at {:#X}", mb_info);
	let page_address = align_down!(mb_info, BasePageSize::SIZE);
	paging::map::<BasePageSize>(page_address, page_address, 1, PageTableEntryFlags::empty());

	// Load the Multiboot information and identity-map the modules information.
	let modules_address = MULTIBOOT.modules_address().expect("Could not find module information in the Multiboot information");
	let page_address = align_down!(modules_address, BasePageSize::SIZE);
	paging::map::<BasePageSize>(page_address, page_address, 1, PageTableEntryFlags::empty());

	// Iterate through all modules.
	// Collect the start address of the first module and the highest end address of all modules.
	let modules = MULTIBOOT.modules().unwrap();
	let mut found_module = false;
	let mut start_address = 0;
	let mut end_address = 0;

	for m in modules {
		found_module = true;

		if start_address == 0 {
			start_address = m.start_address();
		}

		if m.end_address() > end_address {
			end_address = m.end_address();
		}
	}

	// Memory after the highest end address is unused and available for the physical memory manager.
	// However, we want to move the HermitCore Application to the next 2 MB boundary.
	// So add this boundary and align up the address to be on the safe side.
	end_address += LargePageSize::SIZE;
	physicalmem::init(align_up!(end_address, LargePageSize::SIZE));

	// Identity-map the ELF header of the first module.
	assert!(found_module, "Could not find a single module in the Multiboot information");
	assert!(start_address > 0);
	loaderlog!("Found an ELF module at {:#X}", start_address);
	let page_address = align_down!(start_address, BasePageSize::SIZE);
	paging::map::<BasePageSize>(page_address, page_address, 1, PageTableEntryFlags::empty());

	start_address
}

pub unsafe fn move_kernel(physical_address: usize, virtual_address: usize, file_size: usize) -> usize {
	// We want to move the application to the next 2 MB boundary to let the HermitCore kernel map it as a large page.
	// First calculate the displacement and map a range large enough to span the original code and its new location.
	let new_physical_address = align_up!(physical_address, LargePageSize::SIZE);
	let displacement = new_physical_address - physical_address;
	let page_count = (file_size + displacement) / BasePageSize::SIZE;
	paging::map::<BasePageSize>(virtual_address, physical_address, page_count, PageTableEntryFlags::WRITABLE);

	// Now copy the code byte-wise to the new region, starting from the upper bytes.
	for i in (0..file_size).rev() {
		*((virtual_address + displacement + i) as *mut u8) = *((virtual_address + i) as *const u8);
	}

	// Remap the virtual address to the new physical address.
	let page_count = file_size / BasePageSize::SIZE;
	paging::map::<BasePageSize>(virtual_address, new_physical_address, page_count, PageTableEntryFlags::WRITABLE);

	new_physical_address
}

pub unsafe fn boot_kernel(new_physical_address: usize, virtual_address: usize, mem_size: usize, entry_point: usize) {
	// Supply the parameters to the HermitCore application.
	*((virtual_address + HERMIT_KERNEL_OFFSET_BASE) as *mut usize) = new_physical_address;
	*((virtual_address + HERMIT_KERNEL_OFFSET_IMAGE_SIZE) as *mut usize) = mem_size;
	*((virtual_address + HERMIT_KERNEL_OFFSET_UARTPORT) as *mut usize) = SERIAL_PORT_ADDRESS as usize;

	if let Some(address) = MULTIBOOT.command_line_address() {
		// Identity-map the command line.
		let page_address = align_down!(address, BasePageSize::SIZE);
		paging::map::<BasePageSize>(page_address, page_address, 1, PageTableEntryFlags::empty());

		let cmdline = MULTIBOOT.command_line().unwrap();
		*((virtual_address + HERMIT_KERNEL_OFFSET_CMDLINE) as *mut usize) = address;
		*((virtual_address + HERMIT_KERNEL_OFFSET_CMDSIZE) as *mut usize) = cmdline.len();
	}

	// Jump to the kernel entry point and provide the Multiboot information to it.
	loaderlog!("Jumping to HermitCore Application Entry Point at {:#X}", entry_point);
	asm!("jmp *$0" :: "r"(entry_point), "{rdx}"(mb_info) : "memory" : "volatile");
}
