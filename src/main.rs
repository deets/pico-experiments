//! This example toggles the GPIO25 pin, using a PIO program.
//!
//! If a LED is connected to that pin, like on a Pico board, the LED should blink.
#![no_std]
#![no_main]

use hal::gpio::{FunctionPio0, Pin};
use hal::pac;
use hal::pio::PIOExt;
use hal::Sio;
use panic_probe as _;
use rp2040_hal as hal;
use defmt_rtt as _; // defmt transport

/// The linker will place this boot block at the start of our program image. We
/// need this to help the ROM bootloader get our code up and running.
/// Note: This boot block is not necessary when using a rp-hal based BSP
/// as the BSPs already perform this step.
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

/// Entry point to our bare-metal application.
///
/// The `#[rp2040_hal::entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables and the spinlock are initialised.
///
/// The function configures the RP2040 peripherals, then blinks an LED using the PIO peripheral.
#[rp2040_hal::entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();

    let sio = Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // configure LED pin for Pio0.
    let _led_pin: Pin<_, FunctionPio0> = pins.gpio17.into_mode();
    let _other_pin: Pin<_, FunctionPio0> = pins.gpio18.into_mode();
    // PIN id for use inside of PIO
    let led_pin_id = 17;
    let other_pin = 18;

    // Define some simple PIO program.
    let x_controlled_pulse = pio_proc::pio_asm!(
        "pull block",
        "mov x, osr",
        ".wrap_target",
        "mov y, x",
        "set pins, 1",
        "high:",
        "jmp y-- high",
        "mov y, x",
        "set pins, 0",
        "low:",
        "jmp y-- low",
        "mov isr, x",
        "push noblock",
        ".wrap"
    );

    let long_high = pio_proc::pio_asm!(
        ".wrap_target",
        "set pins, 1 [31]",
        "set pins, 0 [3]",
        ".wrap"
    );


    // Initialize and start PIO
    let (mut pio, sm0, sm1, _, _) = pac.PIO0.split(&mut pac.RESETS);
    let x_controlled_pulse_installed = pio.install(&x_controlled_pulse.program).unwrap();
    let long_high_installed = pio.install(&long_high.program).unwrap();
    let (int, frac) = (0, 0); // as slow as possible (0 is interpreted as 65536)
    let (mut sm0, mut rx0, mut tx0) = rp2040_hal::pio::PIOBuilder::from_program(x_controlled_pulse_installed)
        .set_pins(other_pin, 1)
        .clock_divisor_fixed_point(int, frac)
        .build(sm0);

    let (mut sm1, _, _) = rp2040_hal::pio::PIOBuilder::from_program(long_high_installed)
        .set_pins(led_pin_id, 1)
        .clock_divisor_fixed_point(int, frac)
        .build(sm1);

    sm0.set_pindirs([(other_pin, hal::pio::PinDir::Output)]);
    sm0.start();


    sm1.set_pindirs([(led_pin_id, hal::pio::PinDir::Output)]);
    sm1.start();
    tx0.write(10u32);
    // PIO runs in background, independently from CPU
    loop {
        if let Some(value) = rx0.read() {
            defmt::error!("got fifo value: {}", value);
        }
    }
}
