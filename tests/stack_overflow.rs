#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use lazy_static::lazy_static;
use osdev_rust::{
    exit_qemu, gdt::DOUBLE_FAULT_IST_INDEX, serial_print, serial_println, test_panic_handler,
    QemuExitCode,
};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    osdev_rust::gdt::init();
    init_test_idt();

    stack_overflow();

    panic!("Execution was to stop");
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    // o endereço de retorno é puxado para pilha em cada chamada.
    // causa um stack overflow.
    // isso chega na "guard page" que é criada pelo bootloader.
    // essa guard page não permite leitura nem escrita.
    // e fica abaixo da stack.
    // ao acessarmos a guard page, ocorre um page fault.
    // quando isso acontece, a CPU encontra o handler no IDT e tenta puxar um stack frame específico de interrupts.
    // no entanto, o ponteiro de pilha está apontando ainda para o guard page.
    // isso causa um segundo page fault.
    // quando ocorre um page fault seguido de outro, ocorre um double fault.
    // se não configurarmos uma pilha separada nesse caso para o handling do double fault, ocorre um triple fault e
    // na maioria dos sistemas isso causará um reboot.
    stack_overflow();

    // garante que o compilador não transforme a recursão em um loop.
    // devido a tail recursion optimizations.
    // isso quebraria o teste.
    volatile::Volatile::new(0).read();
}

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        unsafe {
            // quando executa um double fault devido a um stack overflow,
            // a pilha vai está corrompida.
            // por isso armazenamos uma stack separada para o handling de double fault
            // fazemos isso configurando um ponteiro para um stack separada dentro do IST dentro do TSS.
            idt.double_fault
                .set_handler_fn(test_double_flat_handler)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

pub fn init_test_idt() {
    TEST_IDT.load();
}

extern "x86-interrupt" fn test_double_flat_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    // a CPU não permite retorno de handlers de double faults.
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info);
}
