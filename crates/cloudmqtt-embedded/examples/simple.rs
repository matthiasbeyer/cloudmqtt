//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#![no_std]
#![no_main]

use cloudmqtt_embedded::CloudmqttClient;
use cloudmqtt_embedded::Subscription;
use cloudmqtt_embedded::macros::subscription;
use cyw43_pio::PioSpi;
use embassy_net::StackResources;
use embassy_net::tcp::TcpSocket;
use embassy_net::dns::DnsQueryType;
use embassy_rp::gpio::Output;
use embassy_time::Timer;
use mqtt_format::v5::qos::QualityOfService;
use static_cell::StaticCell;

const WIFI_NETWORK: &str = env!("WIFI_NETWORK");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

const MQTT_BROKER_ADDR: &str = env!("MQTT_BROKER_ADDR");
const MQTT_BROKER_PORT: u16 = match u16::from_str_radix(env!("MQTT_BROKER_PORT"), 10) {
    Err(_error) => panic!("MQTT_BROKER_PORT is not a valid u16"),
    Ok(port) => port,
};

const MQTT_USER: &str = env!("MQTT_USER");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");
const MQTT_CLIENT_ID: &str = env!("MQTT_CLIENT_ID");

static NETWORK_STACK_RESOURCES: StaticCell<StackResources<6>> = StaticCell::new();
static NETWORK_STATE: StaticCell<cyw43::State> = StaticCell::new();

static FIRMWARE_FW: &[u8] = include_bytes!(env!("CYW43_FIRMWARE_BIN"));
static FIRMWARE_CLM: &[u8] = include_bytes!(env!("CYW43_FIRMWARE_CLM_BIN"));

pub const SUBSCRIPTIONS: [Subscription; 1] = [subscription! {
    topic: "test",
    qos: QualityOfService::AtLeastOnce,
    retain: false,
}];

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, embassy_rp::peripherals::PIO0, 0, embassy_rp::peripherals::DMA_CH1>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

// just as an compile-example
#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let config = embassy_rp::config::Config::default();
    let p = embassy_rp::init(config);
    let state = NETWORK_STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, FIRMWARE_FW).await;
    spawner.spawn(cyw43_task(runner)).unwrap();

    control.init(FIRMWARE_CLM).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    // Configure network stack
    let config = embassy_net::Config::dhcpv4(Default::default());

    // Init network stack
    let (network_stack, runner) = embassy_net::new(
        net_device,
        config,
        NETWORK_STACK_RESOURCES.init(StackResources::new()),
        0,
    );

    // Launch network task
    spawner.spawn(net_task(runner)).unwrap();

    loop {
        match control
            .join(WIFI_NETWORK, cyw43::JoinOptions::new(WIFI_PASSWORD.as_bytes()))
            .await
        {
            Ok(_) => break,
            Err(err) => {
                defmt::info!("join failed with status={}", err.status);
            }
        }
    }

    defmt::info!("waiting for DHCP...");
    while !network_stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    defmt::info!("DHCP is now up!");

    defmt::info!("waiting for link up...");
    while !network_stack.is_link_up() {
        Timer::after_millis(500).await;
    }
    defmt::info!("Link is up!");

    // Wait for the tap interface to be up before continuing
    defmt::info!("waiting for stack to be up...");
    network_stack.wait_config_up().await;
    defmt::info!("Stack is up!");

    let mqtt_stack_resources: MqttStackResources<10, 10> = MqttStackResources::new();

    let mut tcp_socket = TcpSocket::new(
        network_stack,
        &mut mqtt_stack_resources.rx_buffer,
        &mut mqtt_stack_resources.tx_buffer,
    );
    tcp_socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));
    tcp_socket.set_keep_alive(Some(embassy_time::Duration::from_secs(5)));

    let addrs = network_stack
        .dns_query(MQTT_BROKER_ADDR, DnsQueryType::A)
        .await
        .map_err(|error| {
            defmt::error!(
                "Failed to run DNS query for {}: {:?}",
                MQTT_BROKER_ADDR,
                error
            );

            MqttClientError::RunDns(error)
        })?;

    if addrs.is_empty() {
        defmt::error!("Failed to resolve DNS {}", MQTT_BROKER_ADDR);
        return Err(MqttClientError::ResolveDns);
    }

    let mqtt_addr = addrs[0];
    defmt::info!(
        "connecting to MQTT Broker: {}:{}",
        mqtt_addr,
        MQTT_BROKER_PORT
    );

    let client = CloudmqttClient::new(MQTT_BROKER_ADDR, MQTT_BROKER_PORT, SUBSCRIPTIONS, &mqtt_stack_resources, tcp_socket);

    let mut client = match {
        Ok(c) => {
            defmt::info!("MQTT Client started successfully");
            c
        },
        Err(error) => {
            defmt::error!("Error starting MQTT client: {}", Debug2Format(&error));
            loop {
                Timer::after_secs(60).await
            }
        }
    };

    loop {
        let Some(action) = client.get_next_action() else {
            defmt::debug!("No action got from MQTT client, sleeping 1 sec");
            Timer::after_secs(1).await
        };

        match action {
        }

        defmt::debug!("Received action from mqtt client: {}", action);
    }
}
