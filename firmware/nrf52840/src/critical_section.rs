struct SingleCore;
critical_section::set_impl!(SingleCore);

unsafe impl critical_section::Impl for SingleCore {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let masked = cortex_m::register::primask::read_raw() != 0;
        cortex_m::interrupt::disable();
        masked
    }

    unsafe fn release(masked: critical_section::RawRestoreState) {
        if !masked {
            unsafe { cortex_m::interrupt::enable() };
        }
    }
}
