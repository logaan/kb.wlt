use embassy_futures::select::{Either, select};
use embassy_nrf::{gpio::Output, spim::Spim};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer, with_timeout};
use kb_display::{
    Status, Transport,
    nice_view::{TRANSFER_BYTES, clear_command, encode_frame, idle_command, render},
};

static STATUS: Signal<CriticalSectionRawMutex, Status> = Signal::new();

pub fn update(status: Status) {
    STATUS.signal(status);
}

async fn write(spi: &mut Spim<'_>, cs: &mut Output<'_>, bytes: &[u8]) -> bool {
    cs.set_high();
    Timer::after_micros(6).await;
    let result = matches!(
        with_timeout(Duration::from_millis(100), spi.write(bytes)).await,
        Ok(Ok(()))
    );
    Timer::after_micros(6).await;
    cs.set_low();
    result
}

pub async fn run(mut spi: Spim<'_>, mut cs: Output<'_>) {
    let mut vcom = false;
    let mut status = Status {
        transport: Transport::Bluetooth,
        ..Default::default()
    };
    let mut transfer = [0; TRANSFER_BYTES];
    loop {
        if !write(&mut spi, &mut cs, &clear_command(vcom)).await {
            Timer::after_secs(1).await;
            continue;
        }
        vcom = !vcom;
        let mut redraw = true;
        loop {
            if let Some(next) = STATUS.try_take() {
                status = next;
                redraw = true;
            }
            let success = if redraw {
                encode_frame(&render(status), vcom, &mut transfer);
                write(&mut spi, &mut cs, &transfer).await
            } else {
                write(&mut spi, &mut cs, &idle_command(vcom)).await
            };
            if !success {
                break;
            }
            vcom = !vcom;
            match select(STATUS.wait(), Timer::after_millis(500)).await {
                Either::First(next) => {
                    status = next;
                    redraw = true;
                    Timer::after_millis(50).await;
                }
                Either::Second(_) => redraw = false,
            }
        }
    }
}
