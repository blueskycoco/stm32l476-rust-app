#![no_std]
#![no_main]

#[cfg(feature = "defmt")]
use defmt::*;
#[cfg(feature = "defmt")]
use defmt_rtt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::Peri;
use embassy_stm32::gpio::AnyPin;
use embassy_stm32::gpio::Pull;
use embassy_stm32::spi::{Config, Spi};
use embassy_time::{Timer, Delay};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::flash::{Flash, WRITE_SIZE};
use embassy_stm32::time::Hertz;
use embassy_boot_stm32::{AlignedBuffer, FirmwareUpdater, FirmwareUpdaterConfig};
use embassy_sync::blocking_mutex::Mutex;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_boot_stm32::BlockingFirmwareState;
use embassy_net::{StackResources, Ipv4Cidr, Ipv4Address};
use embassy_net::tcp::TcpSocket;
use embassy_net_enc28j60::Enc28j60;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_io_async::Write;
use core::cell::RefCell;
use static_cell::StaticCell;
use heapless::Vec;
use embassy_stm32::mode::Async;
use panic_reset as _;

#[embassy_executor::task]
async fn net_task(
    mut runner: embassy_net::Runner<
        'static,
        Enc28j60<ExclusiveDevice<Spi<'static, Async>, Output<'static>, Delay>, Output<'static>>,
    >,
) -> ! {
    runner.run().await
}
#[embassy_executor::task]
async fn blinky(pin: Peri<'static, AnyPin>) {
    let mut led = Output::new(pin, Level::High, Speed::Low);

    loop {
        led.set_high();
        Timer::after_millis(1000).await;
        #[cfg(feature = "defmt")]
        info!("led hight");

        led.set_low();
        Timer::after_millis(1000).await;
        #[cfg(feature = "defmt")]
        info!("led low");
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
    spawner.spawn(blinky(p.PC13.into()).unwrap());

    let mut spi_config = Config::default();
    spi_config.frequency = Hertz(20_000_000);
    let cs = Output::new(p.PD7, Level::High, Speed::VeryHigh);
    let rst = Output::new(p.PD6, Level::High, Speed::VeryHigh);
    let spi = Spi::new(p.SPI1, p.PB3, p.PB5, p.PB4, p.DMA1_CH3, p.DMA1_CH2, spi_config);
    let spi = ExclusiveDevice::new(spi, cs, Delay);
    let mac_addr = [2, 3, 4, 5, 6, 7];
    let device = Enc28j60::new(spi, Some(rst), mac_addr);
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 2, 31), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 2, 1)),
    });
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let seed = [0x11, 0x22, 0x33, 0x21, 0x12, 0x89, 0x76, 054];
    let seed = u64::from_le_bytes(seed);
    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);

    spawner.spawn(net_task(runner).unwrap());

    let mut button = ExtiInput::new(p.PE13, p.EXTI13, Pull::Up);
    firmware_state.mark_booted().expect("Failed to mark booted");
    loop {
        button.wait_for_falling_edge().await;
        firmware_state.mark_dfu().expect("Failed to mark dfu");
    }
}
