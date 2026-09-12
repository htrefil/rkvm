use crate::event::Event;
use crate::key::{Key, KeyEvent, Keyboard};

use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant, interval_at,Interval};

pub struct KeyRepeater {
    tx: Sender<(usize,Event)>,
    delay: Duration,
    period: Duration,

    key: Keyboard,
    task: Option<JoinHandle<()>>,
}

impl Drop for KeyRepeater {
    fn drop(&mut self) {
        self.stop();
    }
}

impl KeyRepeater { 
    pub fn new(tx: Sender<(usize,Event)>, key: Keyboard, delay: Duration, period: Duration) -> Self {
        let mut kr = Self {
            tx: tx,
            delay: delay,
            period: period,
            key: key,
            task: None,
        };

        kr.start();
        
        kr
    }

    pub fn key(&mut self, key: Keyboard, down: bool) -> bool {
        if self.key == key {
            if !down {
                self.stop();
                return true;
            }
            return false;
        }

        if down {
            self.key=key;
            self.start();
       }
       return false;
    }


    fn start(&mut self) {
        self.stop();
        
        let start = Instant::now() + self.delay;
        let interval = interval_at(start, self.period);
        self.task = Some(tokio::spawn(run(self.tx.clone(), interval, self.key)));
    }

    fn stop(&mut self) {
        match &self.task {
            None => return,
            Some(t) => {
                t.abort();
                self.task = None;
            }
        }
    }
}

async fn run(tx: Sender<(usize,Event)>, mut interval: Interval, key: Keyboard) {
    loop {
        interval.tick().await;

        let _ = tx.send((1, Event::Key(KeyEvent { key: Key::Key(key), down: true}))).await;
    }
}
