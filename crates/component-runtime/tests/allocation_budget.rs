use kb_component_runtime::{BrailleComponent, BrailleState, LedComponent, LedState};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct MeasuringAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for MeasuringAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(live, Ordering::SeqCst);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: MeasuringAllocator = MeasuringAllocator;

#[test]
fn embedded_components_fit_the_nrf_heap_budget() {
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    {
        let mut component = LedComponent::new(include_bytes!(
            "../../../components/generated/led.core.wasm"
        ))
        .unwrap();
        component.step(LedState::default(), 1000, false).unwrap();
    }
    let led_peak = PEAK.load(Ordering::SeqCst) - baseline;
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    {
        let mut component = BrailleComponent::new(include_bytes!(
            "../../../components/generated/braille.core.wasm"
        ))
        .unwrap();
        component
            .step(BrailleState { held: 1, chord: 1 }, 0x140)
            .unwrap();
    }
    let braille_peak = PEAK.load(Ordering::SeqCst) - baseline;
    eprintln!(
        "LED peak allocations: {led_peak} bytes; Braille peak allocations: {braille_peak} bytes"
    );
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    {
        let mut component = LedComponent::new_chain(&[
            include_bytes!("../../../components/generated/led-dim/stage0/led.core.wasm"),
            include_bytes!("../../../components/generated/led-dim/stage1/led.core.wasm"),
        ])
        .unwrap();
        component.step(LedState::default(), 1000, false).unwrap();
    }
    let chain_peak = PEAK.load(Ordering::SeqCst) - baseline;
    eprintln!("Composed LED peak allocations: {chain_peak} bytes");
    assert!(chain_peak <= 208 * 1024);
    assert!(led_peak <= 160 * 1024);
    assert!(braille_peak <= 160 * 1024);
}
