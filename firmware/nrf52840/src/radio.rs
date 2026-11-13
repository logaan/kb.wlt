use crate::ble_bonds::BondStore;
use embassy_executor::Spawner;
use embassy_nrf::interrupt::{InterruptExt, Priority};
use embassy_nrf::mode::Async;
use embassy_nrf::{Peri, bind_interrupts, pac, peripherals, rng, usb};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
#[cfg(any(feature = "toucan", feature = "prospector-display"))]
use embassy_time::{Duration, with_timeout};
use nrf_sdc::{self as sdc, mpsl};
use static_cell::StaticCell;
use trouble_host::prelude::*;

bind_interrupts!(pub(crate) struct Irqs {
    RNG => rng::InterruptHandler<peripherals::RNG>;
    EGU0_SWI0 => mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => mpsl::ClockInterruptHandler, usb::vbus_detect::InterruptHandler;
    RADIO => mpsl::HighPrioInterruptHandler;
    TIMER0 => mpsl::HighPrioInterruptHandler;
    RTC0 => mpsl::HighPrioInterruptHandler;
    USBD => usb::InterruptHandler<peripherals::USBD>;
});

#[cfg(feature = "prospector-display")]
bind_interrupts!(pub(crate) struct DisplayIrqs {
    SPI2 => embassy_nrf::spim::InterruptHandler<peripherals::SPI2>;
});

pub(crate) type ControllerType = sdc::SoftdeviceController<'static>;
#[cfg(any(feature = "toucan", feature = "prospector-display"))]
pub(crate) type Host<'d> = Stack<'d, ControllerType, DefaultPacketPool>;
pub(crate) type Bonds = Mutex<CriticalSectionRawMutex, BondStore<'static>>;

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static mpsl::MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

pub(crate) fn device_id() -> u64 {
    pac::FICR.deviceid(0).read() as u64 | ((pac::FICR.deviceid(1).read() as u64) << 32)
}

pub(crate) fn address() -> Address {
    let low = pac::FICR.deviceaddr(0).read().to_le_bytes();
    let high = pac::FICR.deviceaddr(1).read().to_le_bytes();
    Address::random([low[0], low[1], low[2], low[3], high[0], high[1] | 0xc0])
}

pub(crate) fn radio(
    spawner: Spawner,
    mpsl_peripherals: mpsl::Peripherals<'static>,
    sdc_peripherals: sdc::Peripherals<'static>,
    random: Peri<'static, peripherals::RNG>,
    flash: Peri<'static, peripherals::NVMC>,
    central_count: u8,
    peripheral_count: u8,
) -> (ControllerType, BondStore<'static>, u32) {
    embassy_nrf::interrupt::RNG.set_priority(Priority::P3);
    embassy_nrf::interrupt::USBD.set_priority(Priority::P3);
    static MPSL: StaticCell<mpsl::MultiprotocolServiceLayer> = StaticCell::new();
    static TIMESLOTS: StaticCell<mpsl::SessionMem<1>> = StaticCell::new();
    let mpsl = MPSL.init(
        mpsl::MultiprotocolServiceLayer::with_timeslots(
            mpsl_peripherals,
            Irqs,
            mpsl::raw::mpsl_clock_lfclk_cfg_t {
                source: mpsl::raw::MPSL_CLOCK_LF_SRC_XTAL as u8,
                rc_ctiv: 0,
                rc_temp_ctiv: 0,
                accuracy_ppm: if cfg!(any(
                    feature = "prospector-display",
                    feature = "toucan-dongle"
                )) {
                    500
                } else {
                    20
                },
                skip_wait_lfclk_started: false,
            },
            TIMESLOTS.init(mpsl::SessionMem::new()),
        )
        .unwrap(),
    );
    spawner.spawn(mpsl_task(mpsl).unwrap());
    static RNG: StaticCell<rng::Rng<'static, Async>> = StaticCell::new();
    let rng = RNG.init(rng::Rng::new(random, Irqs));
    let mut boot = [0; 4];
    rng.blocking_fill_bytes(&mut boot);
    static MEMORY: StaticCell<sdc::Mem<16384>> = StaticCell::new();
    let mut builder = sdc::Builder::new().unwrap();
    if central_count != 0 {
        builder = builder.support_scan().support_central();
    }
    if peripheral_count != 0 {
        builder = builder.support_adv().support_peripheral();
    }
    builder = builder
        .central_count(central_count)
        .unwrap()
        .peripheral_count(peripheral_count)
        .unwrap();
    let controller = builder
        .buffer_cfg(128, 128, 4, 4)
        .unwrap()
        .build(sdc_peripherals, rng, mpsl, MEMORY.init(sdc::Mem::new()))
        .unwrap();
    (
        controller,
        BondStore::new(mpsl::Flash::take(mpsl, flash)),
        u32::from_le_bytes(boot),
    )
}

#[cfg(any(feature = "toucan", feature = "prospector-display"))]
pub(crate) async fn authenticate(
    connection: &Connection<'_, DefaultPacketPool>,
    stack: &Host<'_>,
    bonds: &Bonds,
) -> bool {
    if connection.set_bondable(true).is_err() || connection.request_security().is_err() {
        return false;
    }
    let wait = async {
        loop {
            match connection.next().await {
                ConnectionEvent::Encrypted { security_level, .. } => {
                    return security_level.encrypted();
                }
                ConnectionEvent::PairingComplete {
                    security_level,
                    bond,
                } => {
                    if let Some(bond) = bond {
                        bonds.lock().await.remember(&bond).await;
                    }
                    return security_level.encrypted();
                }
                ConnectionEvent::PairingFailed(_) | ConnectionEvent::Disconnected { .. } => {
                    return false;
                }
                ConnectionEvent::RequestConnectionParams(request) => {
                    let _ = request.accept(None, stack).await;
                }
                _ => {}
            }
        }
    };
    with_timeout(Duration::from_secs(30), wait)
        .await
        .unwrap_or(false)
}
