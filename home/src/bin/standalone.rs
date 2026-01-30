#![no_std]
#![no_main]

extern crate alloc;

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};
use anyhow::Result;
use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_futures::select::{self, Either};
use embassy_net::{DhcpConfig, Runner, Stack, StackResources};
use embassy_time::{Duration, Ticker, WithTimeout};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    interrupt::software::SoftwareInterruptControl, peripherals::WIFI, rng::Rng,
    timer::timg::TimerGroup,
};
use esp_println::logger;
use esp_radio::{
    Controller,
    wifi::{
        self, ClientConfig, CountryInfo, ModeConfig, OperatingClass, WifiController, WifiDevice,
        WifiEvent,
    },
};
use freemdu::device::{self, Action, ActionKind, Date, Property, PropertyKind, Value};
use freemdu_home::{OpticalPort, status_led::StatusLed};
use log::{error, info};
use mcutie::{
    McutieBuilder, McutieReceiver, McutieTask, MqttMessage, PublishBytes, Publishable, Topic,
    homeassistant::{
        AvailabilityState, AvailabilityTopics, Device as HaDevice, Entity, Origin, button::Button,
        sensor::Sensor,
    },
};
use static_cell::StaticCell;

// Interval for publishing device properties and actions
const DEVICE_PUBLISH_INTERVAL: Duration =
    Duration::from_secs(freemdu_home::num_from_env!("DEVICE_PUBLISH_INTERVAL", u64));

// Timeout for device operations (e.g. connection)
const DEVICE_TIMEOUT: Duration = Duration::from_secs(1);

// Delay between Wi-Fi reconnection attempts
const WIFI_RETRY_DELAY: Duration = Duration::from_secs(5);

/// MQTT topic used to report device availability
const STATUS_TOPIC: Topic<&str> = Topic::Device("status");

esp_bootloader_esp_idf::esp_app_desc!();

#[embassy_executor::task]
async fn mqtt_stack_task(
    task: McutieTask<
        'static,
        &'static str,
        PublishBytes<'static, &'static str, AvailabilityState>,
        1,
    >,
) {
    // Move large MQTT task to heap
    Box::pin(task.run()).await;
}

#[embassy_executor::task]
async fn mqtt_message_task(
    receiver: McutieReceiver,
    hostname: String,
    mut port: OpticalPort<'static>,
    mut led: StatusLed<'static>,
) -> ! {
    let mut ticker = Ticker::every(DEVICE_PUBLISH_INTERVAL);
    let mut connected = false;

    loop {
        match select::select(receiver.receive(), ticker.next()).await {
            Either::First(MqttMessage::Connected) => {
                connected = true;
                ticker.reset();
            }
            Either::First(MqttMessage::Disconnected) => {
                connected = false;
            }
            Either::First(MqttMessage::Publish(Topic::Device(topic), payload)) => {
                if let Ok(param) = str::from_utf8(&payload)
                    && let Some((id, "trigger")) = topic.split_once('/')
                    && let Err(err) = trigger_action(&mut port, id, param).await
                {
                    error!("Failed to trigger action: {err:#}");
                }
            }
            Either::Second(()) if connected => {
                let state = match publish_device(&mut port, &hostname).await {
                    Ok(()) => AvailabilityState::Online,
                    Err(err) => {
                        error!("Failed to publish device: {err:#}");

                        AvailabilityState::Offline
                    }
                };

                if let Err(err) = STATUS_TOPIC.with_bytes(&state).publish().await {
                    error!("Failed to publish status: {err:?}");
                }
            }
            _ => {}
        }

        led.set_connected(connected.into());
    }
}

async fn publish_device(port: &mut OpticalPort<'_>, hostname: &str) -> Result<()> {
    let mut dev = connect_to_device(port).await?;
    let dev_kind = dev.kind().to_string();
    let props = dev
        .properties()
        .iter()
        .filter(|prop| prop.kind == PropertyKind::Operation);
    let actions = dev
        .actions()
        .iter()
        .filter(|action| action.kind == ActionKind::Operation);
    let mut vals = Vec::with_capacity(props.clone().count());

    // Query properties first, as publishing them immediately might lead to timeout
    for prop in props.clone() {
        let val = dev
            .query_property(prop)
            .with_timeout(DEVICE_TIMEOUT)
            .await
            .map_err(|err| anyhow::anyhow!("Failed to query property: {err:?}"))??;

        info!("Queried property {prop:?} with value {val:?}");
        vals.push(val);
    }

    for (prop, val) in props.zip(vals) {
        publish_property(prop, &dev_kind, hostname).await?;
        publish_property_value(prop, &val).await?;
        info!("Published property: {prop:?}");
    }

    for action in actions {
        // There's no suitable HA component for actions with parameters
        if action.params.is_none() {
            publish_action(action, &dev_kind, hostname).await?;
            info!("Published action: {action:?}");
        } else {
            info!("Skipped action due to parameters: {action:?}");
        }
    }

    Ok(())
}

async fn publish_property(prop: &Property, dev: &str, hostname: &str) -> Result<()> {
    let unique_id = format!("{}_{}", hostname, prop.id);

    Entity {
        device: HaDevice {
            name: Some(dev),
            ..HaDevice::default()
        },
        origin: Origin::default(),
        object_id: &unique_id,
        unique_id: Some(&unique_id),
        name: prop.name,
        availability: AvailabilityTopics::All([STATUS_TOPIC]),
        state_topic: Some(Topic::Device(format!("{}/value", prop.id)).as_ref()),
        command_topic: None,
        component: Sensor {
            device_class: None,
            state_class: None,
            unit_of_measurement: prop.unit,
        },
    }
    .publish_discovery()
    .await
    .map_err(|err| anyhow::anyhow!("Failed to publish HA sensor: {err:?}"))
}

async fn publish_property_value(prop: &Property, val: &Value) -> Result<()> {
    let topic = Topic::Device(format!("{}/value", prop.id));

    match *val {
        Value::Number(num) => topic.with_display(num).publish().await,
        Value::Bool(val) => {
            topic
                .with_display(if val { "Yes" } else { "No" })
                .publish()
                .await
        }
        Value::String(ref string) => topic.with_display(string).publish().await,
        Value::Duration(dur) => {
            let total_mins = dur.as_secs() / 60;
            let hours = total_mins / 60;
            let mins = total_mins % 60;

            topic
                .with_display(format!("{hours}h {mins}min"))
                .publish()
                .await
        }
        Value::Sensor(_, _) => Ok(()), // Sensor values should not be published
        Value::Date(Date { year, month, day }) => {
            topic
                .with_display(format!("{year}-{month:02}-{day:02}"))
                .publish()
                .await
        }
    }
    .map_err(|err| anyhow::anyhow!("Failed to publish property value: {err:?}"))
}

async fn publish_action(action: &Action, dev: &str, hostname: &str) -> Result<()> {
    let unique_id = format!("{}_{}", hostname, action.id);

    Entity {
        device: HaDevice {
            name: Some(dev),
            ..HaDevice::default()
        },
        origin: Origin::default(),
        object_id: &unique_id,
        unique_id: Some(&unique_id),
        name: action.name,
        availability: AvailabilityTopics::All([STATUS_TOPIC]),
        state_topic: None,
        command_topic: Some(Topic::Device(format!("{}/trigger", action.id)).as_ref()),
        component: Button { device_class: None },
    }
    .publish_discovery()
    .await
    .map_err(|err| anyhow::anyhow!("Failed to publish HA button: {err:?}"))
}

async fn trigger_action(port: &mut OpticalPort<'_>, id: &str, param: &str) -> Result<()> {
    let mut dev = connect_to_device(port).await?;

    let Some(action) = dev.actions().iter().find(|action| action.id == id) else {
        return Err(anyhow::anyhow!("Failed to find action with id {id}"));
    };

    info!("Triggering action {action:?} with parameter {param}");

    let param = if action.params.is_some() {
        Some(Value::String(param.to_string()))
    } else {
        None
    };

    Ok(dev
        .trigger_action(action, param)
        .with_timeout(DEVICE_TIMEOUT)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to trigger action: {err:?}"))??)
}

async fn connect_to_device<'a, 'b>(
    port: &'a mut OpticalPort<'b>,
) -> Result<Box<dyn device::Device<&'a mut OpticalPort<'b>> + 'a>> {
    let dev = device::connect(port)
        .with_timeout(DEVICE_TIMEOUT)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to connect to device: {err:?}"))??;

    info!(
        "Connected to device with kind {} and software ID {}",
        dev.kind(),
        dev.software_id()
    );

    Ok(dev)
}

#[embassy_executor::task]
async fn network_stack_task(mut runner: Runner<'static, WifiDevice<'static>>) -> ! {
    runner.run().await;
}

#[embassy_executor::task]
async fn wifi_connect_task(mut controller: WifiController<'static>) -> ! {
    loop {
        match controller.connect_async().await {
            Ok(()) => {
                info!("Wi-Fi connected");
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                info!("Wi-Fi disconnected");
            }
            Err(err) => {
                error!("Failed to connect to Wi-Fi: {err:?}");
                embassy_time::Timer::after(WIFI_RETRY_DELAY).await;
            }
        }
    }
}

fn init_wifi(wifi: WIFI<'static>) -> Result<(WifiController<'static>, WifiDevice<'static>)> {
    static CONTROLLER: StaticCell<Controller<'_>> = StaticCell::new();

    let controller = CONTROLLER.init(
        esp_radio::init().map_err(|err| anyhow::anyhow!("Failed to initialize radio: {err:?}"))?,
    );
    let (mut controller, intfs) = wifi::new(
        controller,
        wifi,
        wifi::Config::default().with_country_code(
            CountryInfo::from(*b"01").with_operating_class(OperatingClass::Indoors),
        ),
    )
    .map_err(|err| anyhow::anyhow!("Failed to create Wi-Fi controller: {err:?}"))?;

    controller
        .set_config(&ModeConfig::Client(
            ClientConfig::default()
                .with_ssid(env!("WIFI_SSID").into())
                .with_password(env!("WIFI_PASSWORD").into()),
        ))
        .map_err(|err| anyhow::anyhow!("Failed to set Wi-Fi configuration: {err:?}"))?;
    controller
        .start()
        .map_err(|err| anyhow::anyhow!("Failed to start Wi-Fi controller: {err:?}"))?;

    Ok((controller, intfs.sta))
}

fn hostname_from_wifi(dev: &WifiDevice<'_>) -> Result<String> {
    let mut hostname = String::with_capacity(32);

    write!(&mut hostname, "freemdu_home_")?;

    for byte in dev.mac_address() {
        write!(&mut hostname, "{byte:02x}")?;
    }

    Ok(hostname)
}

fn init_network(
    dev: WifiDevice<'static>,
    hostname: &str,
) -> Result<(Stack<'static>, Runner<'static, WifiDevice<'static>>)> {
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

    let resources = RESOURCES.init(StackResources::new());
    let rng = Rng::new();
    let seed = (u64::from(rng.random()) << 32) | u64::from(rng.random());
    let mut cfg = DhcpConfig::default();

    cfg.hostname = Some(
        hostname
            .try_into()
            .map_err(|err| anyhow::anyhow!("Failed to set DHCP hostname: {err:?}"))?,
    );

    Ok(embassy_net::new(
        dev,
        embassy_net::Config::dhcpv4(cfg),
        resources,
        seed,
    ))
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    logger::init_logger_from_env();

    let peripherals = esp_hal::init(esp_hal::Config::default());

    esp_alloc::heap_allocator!(size: 128 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    let port = freemdu_home::new_optical_port(peripherals.UART1).unwrap();
    let led = freemdu_home::new_status_led(
        #[cfg(feature = "ws2812led")]
        peripherals.RMT,
    );
    let (wifi_controller, wifi_dev) = init_wifi(peripherals.WIFI).unwrap();
    let hostname = hostname_from_wifi(&wifi_dev).unwrap();
    let (net_stack, net_runner) = init_network(wifi_dev, &hostname).unwrap();
    let (mqtt_receiver, mqtt_task) =
        McutieBuilder::new(net_stack, "freemdu_home", env!("MQTT_HOSTNAME"))
            .with_authentication(env!("MQTT_USERNAME"), env!("MQTT_PASSWORD"))
            .with_subscriptions([Topic::Device("+/trigger")])
            .with_last_will(STATUS_TOPIC.with_bytes(AvailabilityState::Offline))
            .build();

    spawner.spawn(mqtt_stack_task(mqtt_task)).unwrap();
    spawner
        .spawn(mqtt_message_task(mqtt_receiver, hostname, port, led))
        .unwrap();
    spawner.spawn(network_stack_task(net_runner)).unwrap();
    spawner.spawn(wifi_connect_task(wifi_controller)).unwrap();
}
