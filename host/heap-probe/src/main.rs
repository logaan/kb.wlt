#![no_std]
#![no_main]
extern crate alloc;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};
use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use kb_component_runtime::KeyboardPipeline;
use kb_core::pipeline::{Input, KeyEvent, KeyState, PipelineState};
use panic_semihosting as _;

struct MeasuringAllocator(embedded_alloc::LlffHeap);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for MeasuringAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { self.0.alloc(layout) };
        if !pointer.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(live, Ordering::SeqCst);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { self.0.dealloc(pointer, layout) };
    }
}
#[global_allocator]
static ALLOCATOR: MeasuringAllocator = MeasuringAllocator(embedded_alloc::LlffHeap::empty());
const HEAP_SIZE: usize = 196 * 1024;

static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[entry]
fn main() -> ! {
    unsafe {
        ALLOCATOR
            .0
            .init(core::ptr::addr_of_mut!(HEAP) as usize, HEAP_SIZE);
    }
    let shift = include_bytes!(env!("KB_KEYBOARD_STAGE0_WASM"));
    let keymap = include_bytes!(env!("KB_KEYBOARD_STAGE1_WASM"));
    let pipeline = KeyboardPipeline::new(&[shift, keymap]).unwrap();
    let mut state = PipelineState::<2, 64>::new();
    let mut now = 0;
    for _ in 0..10 {
        for layer in [false, true] {
            if layer {
                let input = Input::with_events(
                    now,
                    &[KeyEvent {
                        position: 36,
                        state: KeyState::Pressed,
                        timestamp_ms: now,
                    }],
                )
                .unwrap();
                state = pipeline.process::<64, 42, 42>(state, input).unwrap().state;
            }
            for mode in [KeyState::Pressed, KeyState::Released] {
                let events: [KeyEvent; 42] = core::array::from_fn(|position| KeyEvent {
                    position: position as u16,
                    state: mode,
                    timestamp_ms: now,
                });
                let input = Input::with_events(now, &events).unwrap();
                let output = pipeline.process::<64, 42, 42>(state, input).unwrap();
                state = output.state;
                now += 1;
            }
            state = pipeline
                .process::<64, 42, 42>(state, Input::tick(now))
                .unwrap()
                .state;
        }
    }
    hprintln!(
        "ARM keyboard pipeline peak={} live={} budget={}",
        PEAK.load(Ordering::SeqCst),
        LIVE.load(Ordering::SeqCst),
        HEAP_SIZE
    );
    debug::exit(debug::EXIT_SUCCESS);
    loop {
        cortex_m::asm::wfi();
    }
}
