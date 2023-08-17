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
use hal::clocks::{Clock, ClocksManager};


/// The linker will place this boot block at the start of our program image. We
/// need this to help the ROM bootloader get our code up and running.
/// Note: This boot block is not necessary when using a rp-hal based BSP
/// as the BSPs already perform this step.
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;


// Pico external clock
const XTAL_FREQ_HZ: u32 = 12_000_000u32;

/// Entry point to our bare-metal application.
///
/// The `#[rp2040_hal::entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables and the spinlock are initialised.
///
/// The function configures the RP2040 peripherals, then blinks an LED using the PIO peripheral.
#[rp2040_hal::entry]
fn main() -> ! {
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();

    let clocks = ClocksManager::new(pac.CLOCKS);

    let sio = Sio::new(pac.SIO);
    let pins = hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // configure LED pin for Pio0.
    let _trigger_pin: Pin<_, FunctionPio0> = pins.gpio17.into_mode();
    let _echo_pin: Pin<_, FunctionPio0> = pins.gpio18.into_mode();
    // PIN id for use inside of PIO
    let echo_pin = 18;
    let trigger_pin = 17;

    // Define some simple PIO program.
    // I want the pulse of 31 highs to be
    // ~16us. freq / 1000_000 is the number
    // of clocks for 1us, times 16 the number of clocks
    // the whole period should take. Divide by 31 to
    // reach the divider.
    // TODO: currently I need an additional divider 2, why?
    let freq = clocks.system_clock.freq().raw();
    let div = (16 * freq / 1000_000 / 31 / 2) as u16;
    // The spec says that in case no echo is received,
    // we get a 200ms pulse. Let's use that for now to limit
    // the rate. By waiting for 256ms, we can derive our waits
    // simply from the divider that is a 31st of 16us
    let wait_for_echo = (div as u32) * 16 * 1000;
    defmt::error!("divider {} at {}", div, freq);

    let trigger_program = pio_proc::pio_asm!(
        "pull block",
        "mov x, osr",
        ".wrap_target",
        "set pins, 1 [31]",
        "set pins, 0 [31]",
        "mov y !null",
        "wait 1 pin 0",
        "count:",
        "jmp y-- decrement",
        "decrement:",
        "jmp pin count",
        "mov isr y",
        "push block" ,
        ".wrap"
    );


    // Initialize and start PIO
    let (mut pio, trigger_sm, _, _, _) = pac.PIO0.split(&mut pac.RESETS);
    let trigger_installed = pio.install(&trigger_program.program).unwrap();
    let (int, frac) = (div, 0); // as slow as possible (0 is interpreted as 65536)
    let (mut trigger_sm, mut trigger_rx, mut trigger_tx) = rp2040_hal::pio::PIOBuilder::from_program(trigger_installed)
        .set_pins(trigger_pin, 1)
        .in_pin_base(echo_pin)
        .jmp_pin(echo_pin)
        .clock_divisor_fixed_point(int, frac)
        .build(trigger_sm);

    trigger_sm.set_pindirs([(trigger_pin, hal::pio::PinDir::Output), (echo_pin, hal::pio::PinDir::Input)]);
    trigger_sm.start();

    // PIO runs in background, independently from CPU
    trigger_tx.write(wait_for_echo);
    loop {
        if let Some(value) = trigger_rx.read() {
            // We have two instructions in the count loop,
            // thus the total number of spent instructions
            let elapsed_instruction_count = (u32::MAX - value) * 2;
            let elapsed_clock_cycles = elapsed_instruction_count * div as u32;
            let elapsed_seconds = elapsed_clock_cycles as f32 / freq as f32;
            defmt::error!("seconds: {}, centimeters: {}", elapsed_seconds, elapsed_seconds * 330.0 * 100.0);
        }
    }
}
