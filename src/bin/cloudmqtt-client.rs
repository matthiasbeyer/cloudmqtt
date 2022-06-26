use clap::Parser;
use cloudmqtt::{MqttClient, MqttConnectionParams};
use mqtt_format::v3::will::MLastWill;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(long, value_parser)]
    addr: String,
    #[clap(long, value_parser)]
    client_id: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut client = MqttClient::connect_v3_unsecured(
        &args.addr,
        MqttConnectionParams {
            clean_session: true,
            will: Some(MLastWill {
                topic: mqtt_format::v3::strings::MString {
                    value: "hello/world",
                },
                payload: b"I died!",
                qos: mqtt_format::v3::qos::MQualityOfService::AtMostOnce,
                retain: false,
            }),
            username: None,
            password: None,
            keep_alive: 100,
            client_id: mqtt_format::v3::strings::MString {
                value: &args.client_id,
            },
        },
    )
    .await
    .unwrap();

    let mut buffer = vec![];

    loop {
        match client.message_listener(&mut buffer).await {
            Ok(packet) => println!("Received: {packet:#?}"),
            Err(e) => {
                eprintln!("Encountered error while parsing packet: {:?}", e);
                break;
            }
        }
    }
}
