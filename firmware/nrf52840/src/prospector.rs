use embassy_nrf::{gpio::Output, spim::Spim};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer, with_timeout};
use kb_display::{
    Status, Transport,
    st7789::{PanelBus, St7789},
};

#[derive(Clone, Copy)]
enum View {
    Status(Status),
    Reference(kb_core::reference::Page),
}

static VIEW: Signal<CriticalSectionRawMutex, View> = Signal::new();

pub fn update(status: Status) {
    VIEW.signal(View::Status(status));
}

pub fn show_reference(page: kb_core::reference::Page) {
    VIEW.signal(View::Reference(page));
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
    let mut panel = St7789::new(Bus { spi, cs, dc, reset });
    let mut view = View::Status(Status {
        transport: Transport::Split,
        ..Status::default()
    });
    loop {
        backlight.set_high();
        if !matches!(
            with_timeout(Duration::from_secs(2), panel.initialize()).await,
            Ok(Ok(()))
        ) {
            Timer::after_secs(1).await;
            continue;
        }
        backlight.set_low();
        loop {
            if let Some(next) = VIEW.try_take() {
                view = next;
            }
            let present = async {
                match view {
                    View::Status(status) => panel.present(status).await,
                    View::Reference(page) => panel.present_reference(&page).await,
                }
            };
            if !matches!(
                with_timeout(Duration::from_secs(2), present).await,
                Ok(Ok(()))
            ) {
                break;
            }
            view = VIEW.wait().await;
            Timer::after_millis(50).await;
        }
    }
}
