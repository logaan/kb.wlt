use core::sync::atomic::{AtomicU8, Ordering};
use embassy_nrf::{
    Peri, peripherals,
    saadc::{self, ChannelConfig, Saadc},
};
use embassy_time::Timer;

embassy_nrf::bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});

static LEVEL: AtomicU8 = AtomicU8::new(255);

pub fn level() -> u8 {
    LEVEL.load(Ordering::Relaxed)
}

pub async fn monitor(
    adc: Peri<'static, peripherals::SAADC>,
    mut input: ChannelConfig<'static>,
    numerator: u32,
    denominator: u32,
) {
    input.time = saadc::Time::_40US;
    let mut adc = Saadc::new(adc, Irqs, Default::default(), [input]);
    adc.calibrate().await;
    loop {
        let mut total = 0;
        for _ in 0..8 {
            let mut sample = [0];
            adc.sample(&mut sample).await;
            total += u32::from(sample[0].max(0) as u16);
        }
        let mv = (total * 3600 / (8 * 4096)) * numerator / denominator;
        let level = match mv {
            0..2800 | 4410.. => 255,
            _ => kb_core::battery::percentage(mv),
        };
        LEVEL.store(level, Ordering::Relaxed);
        Timer::after_secs(15).await;
    }
}
