use embassy_time::Timer;

#[cfg(any(feature = "toucan-right", feature = "toucan-dongle"))]
use core::sync::atomic::{AtomicBool, Ordering};
#[cfg(any(feature = "toucan-right", feature = "toucan-dongle"))]
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
#[cfg(any(feature = "toucan-right", feature = "toucan-dongle"))]
use trouble_host::{
    att::{AttClient, AttReq},
    prelude::*,
};

#[cfg(any(feature = "toucan-right", feature = "toucan-dongle"))]
static RESET: Signal<CriticalSectionRawMutex, ()> = Signal::new();
#[cfg(any(feature = "toucan-right", feature = "toucan-dongle"))]
static RESET_PENDING: AtomicBool = AtomicBool::new(false);

pub(crate) async fn enter_uf2() -> ! {
    Timer::after_millis(200).await;
    cortex_m::interrupt::disable();
    embassy_nrf::pac::POWER
        .gpregret()
        .write(|value| value.set_gpregret(0x57));
    cortex_m::peripheral::SCB::sys_reset()
}

#[cfg(any(feature = "toucan-right", feature = "toucan-dongle"))]
#[embassy_executor::task]
pub(crate) async fn remote_reset() -> ! {
    RESET.wait().await;
    enter_uf2().await
}

#[cfg(any(feature = "toucan-right", feature = "toucan-dongle"))]
pub(crate) async fn acknowledge(
    write: WriteEvent<'_, '_, DefaultPacketPool>,
    ready: bool,
    release: impl FnOnce(),
) {
    let error = if !ready || RESET_PENDING.load(Ordering::Relaxed) {
        Some(AttErrorCode::INSUFFICIENT_AUTHORISATION)
    } else if !matches!(
        write.payload().incoming(),
        AttClient::Request(AttReq::Write { data: [1], .. })
    ) {
        Some(AttErrorCode::VALUE_NOT_ALLOWED)
    } else {
        None
    };
    if let Some(error) = error {
        if let Ok(reply) = write.reject(error) {
            reply.send().await;
        }
    } else if write
        .accept_unprocessed()
        .is_ok_and(|reply| reply.try_send().is_ok())
    {
        RESET_PENDING.store(true, Ordering::Relaxed);
        release();
        RESET.signal(());
    }
}
