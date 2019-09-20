use crate::log;
use crate::async_utils::TwinBarrier;
use crate::sw_star::SWStar;
use crate::info_handler::InformationHandler;
use tokio::sync::{Lock, mpsc::Sender};
use std::sync::Arc;

pub struct Ticker {
    stars: Lock<Vec<SWStar>>,
    computation_end: TwinBarrier,
    tick_end: TwinBarrier,
    iterations_chan_tx: Sender<usize>,
}

impl Ticker {
    pub fn new(computation_end: TwinBarrier,
           tick_end: TwinBarrier,
           stars: Lock<Vec<SWStar>>,
           info_handler: Arc<InformationHandler>) -> Ticker {
        Ticker {
            computation_end,
            tick_end,
            stars,
            iterations_chan_tx: info_handler.get_iterations_sender(),
        }
    }

    pub async fn tick(&mut self) {
        let _log = log::get_root_logger();
        loop {
            self.computation_end.wait().await;
            {
                let stars_l = self.stars.lock().await;
                let mut iterations = 0;
                stars_l.iter().for_each(|sw| {
                    if let Some(samps) = sw.star.samples.as_ref() {
                        let tick_index = { *sw.star.samples_tick_index.borrow() };

                        if tick_index < samps.len() {
                            sw.tick(samps[tick_index]);
                            iterations += 1;
                            sw.star.samples_tick_index.replace(tick_index + 1);
                        }
                    }
                });
                match self.iterations_chan_tx.send(iterations).await {
                    _ => () // NOTE for now ignore err b/c non-essential
                };
            }
            self.tick_end.wait().await;
        }
    }
}
