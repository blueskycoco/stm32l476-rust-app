#![no_std]
#![no_main]

#[cfg(feature = "defmt")]
use defmt_rtt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::Peri;
use embassy_stm32::gpio::AnyPin;
use embassy_stm32::gpio::Pull;
use embassy_time::Timer;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::flash::{Flash, WRITE_SIZE};
use embassy_boot_stm32::{AlignedBuffer, FirmwareUpdater, FirmwareUpdaterConfig};
use embassy_sync::blocking_mutex::Mutex;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_boot_stm32::BlockingFirmwareState;
use core::cell::RefCell;
use panic_reset as _;

#[embassy_executor::task]
async fn blinky(pin: Peri<'static, AnyPin>) {
    let mut led = Output::new(pin, Level::High, Speed::Low);

    loop {
        led.set_high();
        Timer::after_millis(100).await;

        led.set_low();
        Timer::after_millis(100).await;
    }
}
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let flash = Flash::new_blocking(p.FLASH);
    let flash = Mutex::new(RefCell::new(flash));
    let config = FirmwareUpdaterConfig::from_linkerfile_blocking(&flash, &flash);
    let mut magic = AlignedBuffer([0; WRITE_SIZE]);
    let mut firmware_state = BlockingFirmwareState::from_config(config, &mut magic.0);
    firmware_state.mark_booted().expect("Failed to mark booted");
    spawner.spawn(blinky(p.PC13.into()).unwrap());

    let mut button = ExtiInput::new(p.PE13, p.EXTI13, Pull::Up);
    loop {
        button.wait_for_falling_edge().await;
        firmware_state.mark_dfu().expect("Failed to mark dfu");
    }
}
