use anyhow::{Context, Result};
use freemdu::{
    device::{self, Action, DeviceKind, Error, Property, PropertyKind, Value},
    serial::Port,
};
use log::debug;
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task,
    time::{self, Duration},
};

// Timeout for device operations (e.g. connection)
const DEVICE_TIMEOUT: Duration = Duration::from_secs(1);

// Delay between device connection attempts
const DEVICE_CONNECT_INTERVAL: Duration = Duration::from_secs(4);

type Device<'a> = Box<dyn device::Device<&'a mut Port> + 'a>;

#[derive(Debug)]
pub enum Request {
    QueryProperties(PropertyKind),
    TriggerAction(&'static Action, Option<Value>),
}

#[derive(Debug)]
pub enum Response {
    DeviceConnected {
        software_id: u16,
        kind: DeviceKind,
        actions: &'static [Action],
        tx: UnboundedSender<Request>,
    },
    DeviceDisconnected,
    PropertiesQueried(PropertyKind, Vec<(&'static Property, Value)>),
    InvalidActionArgument(&'static Action),
    InvalidActionState(&'static Action),
}

pub struct Worker<'a> {
    dev: Device<'a>,
    tx: &'a UnboundedSender<Response>,
}

impl Worker<'_> {
    pub fn start(mut port: Port) -> UnboundedReceiver<Response> {
        let (tx, rx) = mpsc::unbounded_channel();

        task::spawn_local(async move {
            loop {
                // Connect to device (retry on timeout)
                match time::timeout(DEVICE_TIMEOUT, device::connect(&mut port)).await {
                    Ok(Ok(dev)) => {
                        let mut worker = Worker { dev, tx: &tx };

                        if let Err(err) = worker.run().await {
                            debug!("Error running device worker: {err:#}");
                        }
                    }
                    Ok(Err(err)) => debug!("Error connecting to device: {err:#}"),
                    Err(_) => debug!("Device connection timed out"),
                }

                time::sleep(DEVICE_CONNECT_INTERVAL).await;
            }
        });

        rx
    }

    async fn run(&mut self) -> Result<()> {
        let (dev_tx, mut dev_rx) = mpsc::unbounded_channel();

        self.tx.send(Response::DeviceConnected {
            software_id: self.dev.software_id(),
            kind: self.dev.kind(),
            actions: self.dev.actions(),
            tx: dev_tx,
        })?;

        // Handle incoming commands from device channel
        while let Some(cmd) = dev_rx.recv().await {
            let res = match cmd {
                Request::QueryProperties(kind) => self
                    .query_properties(kind)
                    .await
                    .context("Failed to query properties"),
                Request::TriggerAction(action, param) => self
                    .trigger_action(action, param)
                    .await
                    .context("Failed to trigger action"),
            };

            if res.is_err() {
                self.tx.send(Response::DeviceDisconnected)?;

                return res;
            }
        }

        Ok(())
    }

    async fn query_properties(&mut self, kind: PropertyKind) -> Result<()> {
        let mut data = Vec::new();

        for prop in self
            .dev
            .properties()
            .iter()
            .filter(|prop| prop.kind == kind)
        {
            let val = time::timeout(DEVICE_TIMEOUT, self.dev.query_property(prop)).await??;

            data.push((prop, val));
        }

        self.tx.send(Response::PropertiesQueried(kind, data))?;

        Ok(())
    }

    async fn trigger_action(
        &mut self,
        action: &'static Action,
        param: Option<Value>,
    ) -> Result<()> {
        match time::timeout(DEVICE_TIMEOUT, self.dev.trigger_action(action, param)).await? {
            Err(Error::InvalidArgument) => self.tx.send(Response::InvalidActionArgument(action))?,
            Err(Error::InvalidState) => self.tx.send(Response::InvalidActionState(action))?,
            res => res?,
        }

        Ok(())
    }
}
