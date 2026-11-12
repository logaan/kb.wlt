use embassy_nrf::twim::Twim;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer, with_timeout};
use kb_display::{Ssd1306, Status, render};

static STATUS: Signal<CriticalSectionRawMutex, Status> = Signal::new();

pub fn update(status: Status) {
    STATUS.signal(status);
}

pub async fn run(bus: Twim<'_>) {
    let mut display = Ssd1306::new(bus, 0x3c);
    let mut status = Status::default();
    loop {
        if !matches!(
            with_timeout(Duration::from_millis(200), display.initialize()).await,
            Ok(Ok(()))
        ) {
            Timer::after_secs(1).await;
            continue;
        }
        loop {
            if let Some(next) = STATUS.try_take() {
                status = next;
            }
            let frame = render(status);
            if !matches!(
                with_timeout(Duration::from_millis(200), display.present(&frame)).await,
                Ok(Ok(()))
            ) {
                break;
            }
            status = STATUS.wait().await;
            Timer::after_millis(50).await;
        }
    }
}
