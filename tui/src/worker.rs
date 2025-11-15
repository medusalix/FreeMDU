use anyhow::{Context, Result};
use freemdu::{
    device::{self, Action, DeviceKind, Error, Property, PropertyKind, Value},
    serial::Port,
};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::{self, JoinHandle},
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
    PropertiesQueried(PropertyKind, Vec<(&'static Property, Value)>),
    InvalidActionArgument(&'static Action),
    InvalidActionState(&'static Action),
}

pub struct Worker<'a> {
    dev: Device<'a>,
    tx: UnboundedSender<Response>,
}

impl Worker<'_> {
    pub fn start(mut port: Port) -> (UnboundedReceiver<Response>, JoinHandle<Result<()>>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = task::spawn_local(async move {
            loop {
                // Connect to device (retry on timeout)
                match time::timeout(DEVICE_TIMEOUT, device::connect(&mut port)).await {
                    Ok(dev) => return Worker { dev: dev?, tx }.run().await,
                    Err(_) => time::sleep(DEVICE_CONNECT_INTERVAL).await,
                }
            }
        });

        (rx, handle)
    }

    async fn run(&mut self) -> Result<()> {
        let (tx, mut rx) = mpsc::unbounded_channel();

        self.tx.send(Response::DeviceConnected {
            software_id: self.dev.software_id(),
            kind: self.dev.kind(),
            actions: self.dev.actions(),
            tx,
        })?;

        // Handle incoming commands from session channel
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Request::QueryProperties(kind) => self
                    .query_properties(kind)
                    .await
                    .context("Failed to query properties")?,
                Request::TriggerAction(action, param) => self
                    .trigger_action(action, param)
                    .await
                    .context("Failed to trigger action")?,
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
