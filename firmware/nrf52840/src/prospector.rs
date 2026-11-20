use embassy_nrf::{gpio::Output, spim::Spim};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use kb_display::{
    Scene,
    st7789::{GraphicsMemory, GraphicsPanel, PanelBus},
};

static VIEW: Signal<CriticalSectionRawMutex, Scene> = Signal::new();

pub fn present(scene: Scene) {
    VIEW.signal(scene);
}

struct Bus<'a> {
    spi: Spim<'a>,
    cs: Output<'a>,
    dc: Output<'a>,
    reset: Output<'a>,
}

impl PanelBus for Bus<'_> {
    type Error = embassy_nrf::spim::Error;

    async fn reset(&mut self) {
        self.cs.set_high();
        self.reset.set_low();
        Timer::after_millis(10).await;
        self.reset.set_high();
        Timer::after_millis(120).await;
    }

    async fn delay_ms(&mut self, milliseconds: u32) {
        Timer::after_millis(milliseconds as u64).await;
    }

    async fn command(&mut self, command: u8, data: &[u8]) -> Result<(), Self::Error> {
        self.cs.set_low();
        self.dc.set_low();
        let result = self.spi.write(&[command]).await;
        self.dc.set_high();
        let result = match result {
            Ok(()) if !data.is_empty() => self.spi.write(data).await,
            result => result,
        };
        self.cs.set_high();
        result
    }

    async fn data(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.cs.set_low();
        self.dc.set_high();
        let result = self.spi.write(data).await;
        self.cs.set_high();
        result
    }
}

pub async fn run<'a>(
    spi: Spim<'a>,
    cs: Output<'a>,
    dc: Output<'a>,
    reset: Output<'a>,
    mut backlight: Output<'a>,
) {
    static MEMORY: static_cell::ConstStaticCell<GraphicsMemory> =
        static_cell::ConstStaticCell::new(GraphicsMemory::EMPTY);
    let mut panel = GraphicsPanel::new(Bus { spi, cs, dc, reset }, MEMORY.take());
    let mut view = Scene::EMPTY;
    loop {
        backlight.set_low();
        if !matches!(
            with_timeout(Duration::from_secs(2), panel.initialize()).await,
            Ok(Ok(()))
        ) {
            Timer::after_secs(1).await;
            continue;
        }
        backlight.set_high();
        let mut previous = None;
        loop {
            if let Some(next) = VIEW.try_take() {
                view = next;
            }
            let next_frame = Instant::now() + Duration::from_hz(30);
            if !matches!(
                with_timeout(
                    Duration::from_secs(2),
                    panel.present(previous.as_ref(), &view)
                )
                .await,
                Ok(Ok(()))
            ) {
                break;
            }
            previous = Some(view);
            view = VIEW.wait().await;
            Timer::at(next_frame).await;
        }
    }
}
