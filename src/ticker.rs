use crate::async_utils::TwinBarrier;
use crate::cli::DetectorOpts;
use crate::gwac_reader::GWACFrame;
use crate::info_handler::InformationHandler;
use crate::log;
use crate::star::{parse_model, Star, StarModelType, StarType};
use crate::sw_star::SWStar;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc::Receiver, mpsc::Sender, Lock};

pub struct Ticker {
    stars: Lock<Vec<SWStar>>,
    computation_end: TwinBarrier,
    tick_end: TwinBarrier,
    iterations_chan_tx: Sender<usize>,
    gwac_rx_chan: Option<Receiver<GWACFrame>>,
    detector_opts: DetectorOpts,
    #[allow(unused)]
    is_offline: bool,
    info_handler: Arc<InformationHandler>,
}

impl Ticker {
    pub fn new(
        computation_end: TwinBarrier,
        tick_end: TwinBarrier,
        stars: Lock<Vec<SWStar>>,
        gwac_rx_chan: Option<Receiver<GWACFrame>>,
        detector_opts: DetectorOpts,
        info_handler: Arc<InformationHandler>,
    ) -> Ticker {
        let iterations_chan_tx = info_handler.get_iterations_sender();
        let is_offline = info_handler.is_offline;
        Ticker {
            computation_end,
            tick_end,
            stars,
            detector_opts,
            gwac_rx_chan,
            info_handler,
            iterations_chan_tx,
            is_offline,
        }
    }

    pub async fn tick(&mut self) {
        let log = log::get_root_logger();
        let sd_rx = self.info_handler.get_shutdown_receiver();
        let mut name_to_pos: HashMap<String, usize> = HashMap::new();
        loop {
            match self.computation_end.wait().await {
                Err(msg) => {
                    if *sd_rx.get_ref() {
                        info!(log, "Ticker received finished signal...");
                        return;
                    } else {
                        panic!(msg)
                    }
                }
                _ => ()
            };
            {
                let mut stars_l = self.stars.lock().await;
                let mut iterations = 0;

                if let Some(gwac_rx_chan) = self.gwac_rx_chan.as_mut() {
                    // NOTE online data handling
                    let mut in_frame = false;
                    let mut tot_stars = 0;
                    loop {
                        // If sender end closes, then file is done for the night
                        // thus, we should shutdown the program gracefully
                        let data = match gwac_rx_chan.recv().await {
                            Some(val) => val,
                            None => {
                                self.info_handler.trigger_shutdown();
                                return;
                            }
                        };

                        match data {
                            GWACFrame::Start => {
                                in_frame = true;
                            }
                            GWACFrame::End => break,
                            // NOTE for now do nothing with file name
                            GWACFrame::Filename(_filename) => continue,
                            GWACFrame::Star(star) => {
                                if !in_frame {
                                    continue;
                                }

                                if !name_to_pos.contains_key(&star.star_id) {
                                    name_to_pos.insert(
                                        star.star_id.clone(),
                                        stars_l.len(),
                                    );

                                    let star = Star {
                                        id: star.star_id.clone(),
                                        uid: star.star_id.clone(),
                                        star_type: StarType::Unknown,
                                        model_type: StarModelType::None,
                                        model: parse_model(
                                            StarModelType::None,
                                            "".to_string(),
                                        ),
                                        sample_rate: 15,
                                        samples: None,
                                        samples_tick_index: RefCell::new(0),
                                    };

                                    stars_l.push(
                                        SWStar::new()
                                            .set_star(star)
                                            .set_availables(
                                                self.detector_opts.fragment,
                                                self.detector_opts.skip_delta,
                                            )
                                            .set_max_buffer_len(100)
                                            .set_window_lens(
                                                self.detector_opts
                                                    .window_length
                                                    .0
                                                    as u32,
                                                self.detector_opts
                                                    .window_length
                                                    .1
                                                    as u32,
                                            )
                                            .build(),
                                    );
                                }

                                stars_l[name_to_pos[&star.star_id]]
                                    .tick(star.mag);
                                tot_stars += 1;
                            }
                        }
                    }

                    debug!(log, ""; "tot_stars_this_read"=>tot_stars.to_string());
                } else {
                    // NOTE offline data handling
                    stars_l.iter().for_each(|sw| {
                        if let Some(samps) = sw.star.samples.as_ref() {
                            let tick_index =
                                { *sw.star.samples_tick_index.borrow() };

                            if tick_index < samps.len() {
                                sw.tick(samps[tick_index]);
                                iterations += 1;
                                sw.star
                                    .samples_tick_index
                                    .replace(tick_index + 1);
                            }
                        }
                    });
                    match self.iterations_chan_tx.send(iterations).await {
                        _ => (), // NOTE for now ignore err b/c non-essential
                    };
                }
            }

            if *sd_rx.get_ref() {
                info!(log, "Ticker received finished signal...");
                return;
            }

            // FIXME Ending early with a Ctrl-C might cause
            // a crash after going through all the produced plots
            // b/c it is waiting here. We could fix this be temporary
            // un-polling this wait and checking sd_rx. For now we will
            // ignore this. It will work properly for a non-forced shutdown.
            match self.tick_end.wait().await {
                Err(msg) => {
                    if *sd_rx.get_ref() {
                        info!(log, "Ticker received finished signal...");
                        return;
                    } else {
                        panic!(msg)
                    }
                }
                _ => ()
            };
        }
    }
}
