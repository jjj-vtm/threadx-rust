use cortex_m_rt::exception;
pub use netx_sys::DMA2_Stream3_IRQHandler as dmi2str3_handler;
pub use netx_sys::SDIO_IRQHandler as sdio_irq_handler;

#[exception]
fn SysTick() {
    unsafe { threadx_rs::tx_timer_interrupt() };
}

#[exception]
fn PendSV() {
    unsafe { threadx_rs::tx_pendsv_handler() };
}

#[exception]
unsafe fn DefaultHandler(irqn: i16) {
    if irqn == 49 {
        sdio_irq_handler();
    } else if irqn == 59 {
        dmi2str3_handler();
    } else {
        defmt::println!("Got interrupt {}", irqn);
    }
    
}
