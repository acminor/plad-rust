use async_std::sync::Mutex;
use tokio::sync as ts;
use crate::log;

struct Event;
struct Data;

pub struct InformationHandler {
    _event_chan: (ts::mpsc::Sender<Event>, ts::mpsc::Receiver<Event>),
    shutdown_chan: (ts::watch::Sender<bool>, ts::watch::Receiver<bool>),
    _ip_data_chan: (ts::mpsc::Sender<Data>, ts::mpsc::Receiver<Data>),
    _org_data_chan: (ts::mpsc::Sender<Data>, ts::mpsc::Receiver<Data>),
    iterations_chan: (ts::mpsc::Sender<usize>, Mutex<ts::mpsc::Receiver<usize>>),
    total_iterations: usize,
    is_offline: bool,
}

impl InformationHandler {
    pub fn new(is_offline: bool, tot_iters: usize) -> InformationHandler {
        let _event_chan = ts::mpsc::channel(16);
        let _ip_data_chan = ts::mpsc::channel(16);
        let _org_data_chan = ts::mpsc::channel(16);
        let iterations_chan = ts::mpsc::channel(16);
        let shutdown_chan = ts::watch::channel(false);
        let total_iterations = tot_iters;

        let iterations_chan = (iterations_chan.0, Mutex::new(iterations_chan.1));

        InformationHandler {
            _event_chan,
            _ip_data_chan,
            _org_data_chan,
            shutdown_chan,
            total_iterations,
            is_offline,
            iterations_chan,
        }
    }

    pub fn get_shutdown_receiver(&self) -> ts::watch::Receiver<bool> {
        self.shutdown_chan.1.clone()
    }

    pub fn get_iterations_sender(&self) -> ts::mpsc::Sender<usize> {
        self.iterations_chan.0.clone()
    }

    pub async fn progress_log(&self) {
        let mut iterations_chan_rx = self.iterations_chan.1.lock().await;
        let total_iterations = self.total_iterations;
        let log = log::get_root_logger();
        let mut iterations = 0;
        let mut log_timer = std::time::Instant::now();
        let now = std::time::Instant::now();
        loop {
            // FIXME do error handling with shutdown chan here at some point
            if let Some(val) = iterations_chan_rx.recv().await {
                iterations += val;
            }

            if iterations == total_iterations && self.is_offline {
                info!(log, "Sending shutdown signal...");
                match self.shutdown_chan.0.broadcast(true) {
                    Ok(_) => (),
                    // TODO look into way to kill entire program from here???
                    _ => panic!("Problem shutting down program..."),
                };
                return;
            }

            if log_timer.elapsed() > std::time::Duration::from_secs(2) {
                let sps = iterations as f32 / now.elapsed().as_secs() as f32;
                let pp =
                    (iterations as f32) / (total_iterations as f32) * 100.0;
                info!(log, "";
                      "TotTime"=>format!("{}s", now.elapsed().as_secs()),
                      "IterationsLeft"=>format!("{}",
                                                total_iterations -
                                                iterations as usize),
                      "EstTimeLeft"=>format!("{}s", (total_iterations -
                                                     iterations as usize)
                                             as f32/sps as f32),
                      "StarsPerSec"=>format!("{}", sps),
                      "StarsPerTenSec"=>format!("{}", sps*10.0),
                      "%Progress"=>format!("{}%", pp));

                log_timer = std::time::Instant::now();
            }
        }
    }
}
