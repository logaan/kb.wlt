use embassy_futures::select::{Either, select};
use embassy_nrf::{gpio::Output, spim::Spim};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer, with_timeout};
use kb_display::{
    Scene,
    toucan_display::{TRANSFER_BYTES, clear_command, encode_scene, idle_command},
};

static VIEW: Signal<CriticalSectionRawMutex, Scene> = Signal::new();

pub fn present(scene: Scene) {
    VIEW.signal(scene);
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
    let mut scene = Scene::EMPTY;
    let mut transfer = [0; TRANSFER_BYTES];
    loop {
        if !write(&mut spi, &mut cs, &clear_command(vcom)).await {
            Timer::after_secs(1).await;
            continue;
        }
        vcom = !vcom;
        let mut redraw = true;
        loop {
            if let Some(next) = VIEW.try_take() {
                scene = next;
                redraw = true;
            }
            let success = if redraw {
                encode_scene(&scene, vcom, &mut transfer);
                write(&mut spi, &mut cs, &transfer).await
            } else {
                write(&mut spi, &mut cs, &idle_command(vcom)).await
            };
            if !success {
                break;
            }
            vcom = !vcom;
            match select(VIEW.wait(), Timer::after_millis(500)).await {
                Either::First(next) => {
                    scene = next;
                    redraw = true;
                    Timer::after_millis(50).await;
                }
                Either::Second(_) => redraw = false,
            }
        }
    }
}
