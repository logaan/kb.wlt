use embassy_nrf::{
    Peri,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    interrupt::{InterruptExt, Priority},
    peripherals::{P0_02, P0_03, P0_04, P0_05, TWISPI0},
    twim::{Config, InterruptHandler, Twim},
};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use kb_client_session::Navigation;

use crate::touch_contact::Contact;

embassy_nrf::bind_interrupts!(struct TouchIrq {
    TWISPI0 => InterruptHandler<TWISPI0>;
});

const ADDRESS: u8 = 0x15;
const CHIP_ID: u8 = 0xa7;
const CST816S: u8 = 0xb4;
const CST816T: u8 = 0xb5;
const CONTACT: u8 = 0x01;
const TIMEOUT: Duration = Duration::from_millis(25);

pub(crate) async fn run(
    mut peripheral: Peri<'_, TWISPI0>,
    interrupt: Peri<'_, P0_02>,
    reset: Peri<'_, P0_03>,
    mut sda: Peri<'_, P0_04>,
    mut scl: Peri<'_, P0_05>,
    route: impl Fn() -> bool,
    dispatch: impl Fn(Navigation),
) {
    let mut interrupt = Input::new(interrupt, Pull::Up);
    let mut reset = Output::new(reset, Level::Low, OutputDrive::Standard);
    Timer::after_millis(5).await;
    reset.set_high();
    Timer::after_millis(50).await;

    embassy_nrf::interrupt::TWISPI0.set_priority(Priority::P3);
    let mut startup = true;
    'recover: loop {
        let mut config = Config::default();
        config.scl_pullup = true;
        config.sda_pullup = true;
        let mut tx_buffer = [0; 1];
        let mut id = [0; 1];
        let mut packet = [0; 6];
        let mut bus = Twim::new(
            peripheral.reborrow(),
            TouchIrq,
            sda.reborrow(),
            scl.reborrow(),
            config,
            &mut tx_buffer,
        );
        let mut detected = false;
        loop {
            let probing_at_boot = core::mem::take(&mut startup);
            if !probing_at_boot {
                interrupt.wait_for_low().await;
            }
            if !detected {
                match with_timeout(TIMEOUT, bus.write_read(ADDRESS, &[CHIP_ID], &mut id)).await {
                    Ok(Ok(())) => detected = matches!(id[0], CST816S | CST816T),
                    Ok(Err(_)) => {}
                    Err(_) => {
                        drop(bus);
                        interrupt.wait_for_high().await;
                        continue 'recover;
                    }
                }
            }
            if probing_at_boot && interrupt.is_high() {
                continue;
            }
            if detected {
                let mut contact = Contact::default();
                loop {
                    match with_timeout(TIMEOUT, bus.write_read(ADDRESS, &[CONTACT], &mut packet))
                        .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(_)) => break,
                        Err(_) => {
                            drop(bus);
                            interrupt.wait_for_high().await;
                            continue 'recover;
                        }
                    }
                    let point = (packet[1] == 1).then(|| {
                        [
                            i16::from_be_bytes([packet[2] & 15, packet[3]]),
                            i16::from_be_bytes([packet[4] & 15, packet[5]]),
                        ]
                    });
                    let now = Instant::now().as_millis() as u32;
                    if let Some(command) = contact.sample(now, point, route()) {
                        dispatch(command);
                    }
                    if !contact.active() {
                        break;
                    }
                    Timer::after_millis(16).await;
                }
            }
            interrupt.wait_for_high().await;
        }
    }
}

#[cfg(not(feature = "toucan"))]
pub(crate) static COMMANDS: embassy_sync::channel::Channel<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    Navigation,
    8,
> = embassy_sync::channel::Channel::new();

#[cfg(feature = "toucan-dongle")]
pub(crate) struct Standalone(kb_client_session::Session);

#[cfg(feature = "toucan-dongle")]
impl Standalone {
    pub(crate) fn new() -> Self {
        let mut session =
            kb_client_session::Session::new(kb_core::preferences::Settings::default());
        session.set_viewport(280, 240);
        session.enable_standalone_receiver_menu();
        session.set_bootloader_devices(kb_client_session::BOOTLOADER_PROSPECTOR);
        Self(session)
    }

    pub(crate) fn model(&self, links: kb_client_session::Links) -> kb_client_session::Model {
        if self.0.menu_open() {
            self.0.model(links)
        } else {
            kb_client_session::Model::home(links)
        }
    }

    pub(crate) async fn update(
        &mut self,
        navigation: Navigation,
        guest: &mut impl kb_client_session::Guest,
    ) {
        let Ok(change) = self.0.navigate_action(navigation, guest) else {
            return;
        };
        if change.bootloader == kb_client_session::BOOTLOADER_PROSPECTOR {
            crate::usb_hid::invalidate_pending_reports();
            crate::bootloader::enter_uf2().await;
        }
    }
}
