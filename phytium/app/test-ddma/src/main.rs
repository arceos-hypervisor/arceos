#![no_std]
#![no_main]

extern crate axplat_aarch64_dyn;

#[macro_use]
extern crate axstd as std;
#[macro_use]
extern crate log;

use core::{
    ffi::CStr,
    sync::atomic::{AtomicBool, Ordering},
};
use std::os::arceos::modules::axhal::irq::register;

use arceos_ext::iomap;
use phytium_ddma::{Channel, ChannelConfig, DDMA, DmaDirection, IrqHandler};
use some_serial::{DataBits, FlowControl, Parity, StopBits, UartConfig, pl011};
use spin::Once;

const DDMA0_BASE: usize = 0x28003000;
const DDMA0_IRQ: usize = 0x6b;
const UARTCLK: usize = 100_000_000; // 100 MHz

const UART0_BASE: usize = 0x2800C000;
const SLAVE_ID_UART0_TX: u8 = 2;
const SLAVE_ID_UART0_RX: u8 = 15;

const BUFF_LEN: usize = 8;

static HANDLE: Once<IrqHandler> = Once::new();
static RSV_COMPLETE: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
fn main() {
    // DMA 从内存搬运到uart0, uart loopback 原样返回, DMA 从uart搬运到内存
    println!("Test DDMA start...");

    let ddma_base = iomap(DDMA0_BASE.into(), 0x1000).unwrap();

    let mut ddma = DDMA::new(ddma_base);

    ddma.reset();

    HANDLE.call_once(|| ddma.irq_handler());

    register(DDMA0_IRQ, || {
        let sts = HANDLE.wait().handle_irq();
        if sts.is_channel_completed(1) {
            RSV_COMPLETE.store(true, Ordering::Release);
        }
    });

    debug!("DDMA controller reset done");

    let uart_virt = iomap(UART0_BASE.into(), 0x1000).unwrap();

    let mut uart = pl011::new(uart_virt, UARTCLK);

    uart.configure(&UartConfig {
        baud_rate: 115200,
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: Parity::None,
        flow_control: FlowControl::None,
    })
    .unwrap();

    uart.loopback_enable();

    uart.enable();

    let rx = uart.try_take_rx().unwrap();
    let tx = uart.try_take_tx().unwrap();

    uart.dma_rx_enable().unwrap();
    uart.dma_tx_enable().unwrap();

    info!("UART initialized");

    let mut channel0 = ddma
        .new_channel(
            0,
            ChannelConfig {
                slave_id: SLAVE_ID_UART0_TX,
                direction: DmaDirection::MemoryToDevice,
                timeout_count: 0x1000,
                blk_size: BUFF_LEN,
                dev_addr: UART0_BASE as _,
                irq: true,
            },
        )
        .unwrap();

    let want = "AB";

    for (i, v) in want.bytes().enumerate() {
        channel0.buff_mut().set(i * 4, v);
    }

    info!("Input buffer: {:?}", channel0.buff().as_ref());

    let mut channel1 = ddma
        .new_channel(
            1,
            ChannelConfig {
                slave_id: SLAVE_ID_UART0_RX,
                direction: DmaDirection::DeviceToMemory,
                timeout_count: 0x1000,
                blk_size: BUFF_LEN,
                dev_addr: UART0_BASE as _,
                irq: true,
            },
        )
        .unwrap();

    ddma.enable();

    info!("DDMA start transfer...");

    channel1.clear_and_active(&mut ddma);
    channel0.clear_and_active(&mut ddma);

    wait_complet(&mut ddma, &mut channel1);

    info!("Output buffer: {:?}", channel1.buff().as_ref());

    let mut str_buff = [0u8; BUFF_LEN / 4 + 1];
    for i in 0..BUFF_LEN / 4 {
        str_buff[i] = channel1.buff().as_ref()[i * 4];
    }

    let s = CStr::from_bytes_until_nul(&str_buff)
        .unwrap()
        .to_string_lossy();

    if !s.contains(want) {
        panic!("Data mismatch: want {want}, got {s}");
    }

    info!("Output string: {s}");
    info!("DMA transfer completed successfully!");

    drop(rx);
    drop(tx);
    // Clear the completion status
    ddma.clear_transfer_complete(channel0.index());
    ddma.clear_transfer_complete(channel1.index());
}

fn wait_complet(ddma: &mut DDMA, channel: &mut Channel) {
    // Wait for transfer completion (polling mode for this test)
    let mut timeout = 100000; // Increase timeout
    while !RSV_COMPLETE.load(Ordering::Acquire) && timeout > 0 {
        timeout -= 1;

        // Add periodic status check
        if timeout % 10000 == 0 {
            debug!("DMA transfer in progress, timeout remaining: {}", timeout);
            debug!("Channel running: {}", channel.is_running());
            if timeout % 50000 == 0 {
                channel.debug_registers();
                ddma.debug_status(channel.index());
            }
        }
        // Small delay to prevent busy waiting
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
    }

    if timeout == 0 {
        info!("DMA transfer timed out");

        // Final debug output
        info!("=== Final Status Debug ===");
        channel.debug_registers();
        ddma.debug_status(channel.index());

        panic!("DMA transfer failed due to timeout");
    }
}
