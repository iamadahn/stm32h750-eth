#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::{IpListenEndpoint, Ipv4Address, Ipv4Cidr, StackResources};
use embassy_stm32::eth::{Ethernet, GenericPhy, PacketQueue};
use embassy_stm32::peripherals::ETH;
use embassy_stm32::rng::Rng;
use embassy_stm32::{bind_interrupts, eth, peripherals, rng, Config};
use embassy_time::Timer;
use embedded_io_async::Write;
use embassy_stm32::time::Hertz;
use static_cell::StaticCell;
use heapless::Vec;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => rng::InterruptHandler<peripherals::RNG>;
});

type Device = Ethernet<'static, ETH, GenericPhy>;

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device>) -> ! {
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    info!("Initialising clocks.");
    let mut config = Config::default();

    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        config.rcc.hsi48 = Some(Default::default());
        config.rcc.hse = Some(Hse {
            freq: Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV5,
            mul: PllMul::MUL192,
            divp: Some(PllDiv::DIV2),
            divq: Some(PllDiv::DIV2),
            divr: Some(PllDiv::DIV2),
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV2;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.apb3_pre = APBPrescaler::DIV2;
        config.rcc.apb4_pre = APBPrescaler::DIV2;
        config.rcc.voltage_scale = VoltageScale::Scale0;
    }

    info!("Initialising hardware.");
    let p = embassy_stm32::init(config);

    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    rng.fill_bytes(&mut seed);
    let seed = u64::from_le_bytes(seed);

    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBA, 0xBE];

    info!("Initialising ethernet.");

    static PACKETS: StaticCell<PacketQueue<4, 4>> = StaticCell::new();
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<4, 4>::new()),
        p.ETH,
        Irqs,
        p.PA1,  // ref_clk
        p.PA2,  // mdio
        p.PC1,  // eth_mdc
        p.PA7,  // CRS_DV: Carrier Sense
        p.PC4,  // RX_D0: Received Bit 0
        p.PC5,  // RX_D1: Received Bit 1
        p.PB12, // TX_D0: Transmit Bit 0
        p.PB13, // TX_D1: Transmit Bit 1
        p.PB11, // TX_EN: Transmit Enable
        GenericPhy::new_auto(),
        mac_addr,
    );

    info!("Ethrenet initialized.");

    //let config = embassy_net::Config::dhcpv4(Default::default());
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 31, 69), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 31, 5)),
    });

    // Init network stack
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);

    // Launch network task
    unwrap!(spawner.spawn(net_task(runner)));

    // Ensure DHCP configuration is up before trying connect
    stack.wait_config_up().await;

    info!("Network task initialized");

    // Then we can use it!
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];

    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

    loop {
        info!("Waiting for connections.");
        let r = socket
            .accept(IpListenEndpoint {
                addr: None,
                port: 80,
            })
        .await;
        info!("Connected.");

        if let Err(e) = r {
            info!("Connection error - {}", e);
        }

        let mut buf = [0u8; 1024];
        let mut pos = 0;
        loop {
            match socket.read(&mut buf).await {
                Ok(0) => {
                    info!("Read EOF.");
                    break;
                }

                Ok(len) => {
                    let to_print = 
                        unsafe { core::str::from_utf8_unchecked(&buf[..(pos + len)]) };
                    if to_print.contains("\r\n\r\n") {
                        info!("Ok(len){}", to_print); 
                        break;
                    }
                    
                    pos += len;
                }

                Err(e) => {
                    info!("Read error - {}.", e);
                    break;
                }
            }
        }

        let r = socket
            .write_all(
                b"HTTP/1.0 200 OK\r\n\r\n\
            <html>\
                <body>\
                    <h1>Hello Rust! Hello STM32!</h1>\
                    <h2>This is my first time ever using ethernet on STM32<h2>\
                </body>\
            </html>\r\n\
                "
                )
            .await;
        if let Err(e) = r {
            info!("Flush error - {}.", e);
        }

        Timer::after_millis(1000).await;
        socket.close();

        Timer::after_millis(1000).await;
        socket.abort();
    }
}

