use crate::abs::{AbsAxis, AbsInfo, AbsEvent};
use crate::event::Event;
use crate::key::{Key, KeyEvent};
use crate::rel::{RelAxis, RelEvent};
use crate::writer::{DeviceWriter, EventWriter};

use crate::windows::key_repeater::KeyRepeater;
use crate::windows::normalizer::AxisNormalizer;

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::ffi::CString;
use std::io::{Error, ErrorKind};
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};

pub struct WriterWindows {
    tx: Sender<(usize,Event)>,
    dev: HashMap<usize,DeviceWriterWindows>,
}

struct DeviceWriterWindows {
    repeat_delay: Duration,
    repeat_period: Duration,
    key_reapeter: Option<KeyRepeater>,

    hi_wheel: bool,
    hi_hwheel: bool,
    abs_norm: HashMap<AbsAxis, AxisNormalizer>,
}

impl WriterWindows {
    pub fn new(mut writer: impl EventWriter + Send +'static) -> Self {
        let (tx, mut rx) = channel(16);
        tokio::spawn(async move {
            while let Some((id, event)) = rx.recv().await {
                if let Err(e) = writer.event(id, event).await {
                    tracing::warn!("Failed to write event {:?}", e);
                }
            }
        });
        WriterWindows { tx: tx, dev: HashMap::new() }
    }
}

async fn send(tx: Sender<(usize,Event)>, id: usize, event: Event) -> Result<(), Error> {
   tx.send((id, event)).await.map_err(|e| Error::new(ErrorKind::BrokenPipe, e.to_string()))
}

impl DeviceWriterWindows {

    pub async fn event(&mut self, tx: Sender<(usize,Event)>, id: usize, event: Event) -> Result<(),Error> {
        match event {
            Event::Key(KeyEvent { key, down }) => {
                match key {
                    Key::Key(key) => {
                        match (&mut self.key_reapeter,down) {
                            (Some(kr), _) => {
                                if kr.key(key, down) {
                                    self.key_reapeter = None;
                                }
                            }
                            (None, true) => self.key_reapeter = Some(KeyRepeater::new(tx.clone(), key, self.repeat_delay, self.repeat_period)),
                            (_,_) => {}
                        }
                        send(tx, id, event).await?
                    }
                    _ => send(tx, id, event).await?
                }
            }
            Event::Rel(RelEvent { axis, value: _ }) => {
                match axis {
                    RelAxis::X => send(tx, id, event).await?,
                    RelAxis::Y => send(tx, id, event).await?,
                    RelAxis::Wheel => if !self.hi_wheel {
                        send(tx, id, event).await?
                    },
                    RelAxis::HWheel => if !self.hi_hwheel {
                        send(tx, id, event).await?
                    },
                    RelAxis::WheelHiRes => if self.hi_wheel {
                        send(tx, id, event).await?
                    },
                    RelAxis::HWheelHiRes => if self.hi_hwheel {
                        send(tx, id, event).await?
                    },
                    _ => tracing::info!("Axis not handled {:?}", event),
                }
            }
            Event::Abs(event) => {
                match event {
                    AbsEvent::Axis { axis, value } => {
                         if let Some(normalizer) = self.abs_norm.get(&axis) {
                            let nv = normalizer.normalize(value);
                            send(tx, id, Event::Abs(AbsEvent::Axis { axis: axis, value: nv})).await?
                        } else {
                            tracing::warn!("Abs Axis not handled: {:?}", axis);
                        }
                    },
                    _ => tracing::warn!("Abs event not handled: {:?}", event),
                }
            }
            _ => send(tx, id, event).await?
        }
        Ok(())
    }
}

#[async_trait]
impl DeviceWriter for WriterWindows {
    async fn create_device(&mut self, id: usize, _name: &CString, _vendor: u16, _product: u16, _version: u16, rel: HashSet<RelAxis>, abs: HashMap<AbsAxis, AbsInfo>, _keys: HashSet<Key>, delay: Option<i32>, period: Option<i32>) -> Result<(), Error> {
        let entry = self.dev.entry(id);
        if let Entry::Occupied(_) = entry {
            return Err(Error::new(ErrorKind::InvalidData, "Server created the same device twice"));
        }

        let mut hi_wheel = false;
        let mut hi_hwheel = false;
        let mut abs_norm = HashMap::new();

        for axis in rel {
            match axis {
                RelAxis::WheelHiRes => hi_wheel = true,
                RelAxis::HWheelHiRes => hi_hwheel = true,
                _ => {},
            }
        }

        abs.into_iter().for_each(|(axis, info)| {
            let normalizer = AxisNormalizer::new(info.min, info.max);
            abs_norm.insert(axis, normalizer);
        });

        let repeat_delay = Duration::from_millis(delay.map_or(0, |x| if x<0 { 0 } else { x }) as u64);
        let repeat_period = Duration::from_millis(period.map_or(0, |x| if x<0  { 0 } else { x }) as u64);

        entry.or_insert(DeviceWriterWindows {
            repeat_delay: repeat_delay,
            repeat_period: repeat_period,
            key_reapeter: None,

            hi_wheel: hi_wheel,
            hi_hwheel: hi_hwheel,
            abs_norm: abs_norm,
        });

        Ok(())
    }
    async fn destroy_device(&mut self, id: usize) -> Result<(), Error> {
        if self.dev.remove(&id).is_none() {
            return Err(Error::new(ErrorKind::InvalidData, "Server destroyed a nonexistent device"));
        }

        tracing::info!(id = %id, "Destroyed device");
        Ok(())
    }
}

#[async_trait]
impl EventWriter for WriterWindows {
    async fn event(&mut self, id: usize, event: Event) -> Result<(), Error> {
        let dev = self.dev.get_mut(&id).ok_or_else(|| { Error::new(ErrorKind::InvalidData,"Server sent an event to a nonexistent device",)})?;
        dev.event(self.tx.clone(), id, event).await
    }
}
