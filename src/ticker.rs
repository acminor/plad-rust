use crate::log;
use crate::async_utils::TwinBarrier;
use crate::sw_star::SWStar;
use crate::info_handler::InformationHandler;
use crate::cli::DetectorOpts;
use crate::gwac_reader::GWACFrame;
use crate::star::{Star, StarType, StarModelType, parse_model};
use tokio::sync::{Lock, mpsc::Sender, mpsc::Receiver};
use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::Arc;

pub struct Ticker {
    stars: Lock<Vec<SWStar>>,
    computation_end: TwinBarrier,
    tick_end: TwinBarrier,
    iterations_chan_tx: Sender<usize>,
    gwac_rx_chan: Option<Receiver<GWACFrame>>,
    detector_opts: DetectorOpts,
    is_offline: bool,
}

impl Ticker {
    pub fn new(computation_end: TwinBarrier,
               tick_end: TwinBarrier,
               stars: Lock<Vec<SWStar>>,
               gwac_rx_chan: Option<Receiver<GWACFrame>>,
               detector_opts: DetectorOpts,
               info_handler: Arc<InformationHandler>) -> Ticker {
        Ticker {
            computation_end,
            tick_end,
            stars,
            detector_opts,
            gwac_rx_chan,
            iterations_chan_tx: info_handler.get_iterations_sender(),
            is_offline: info_handler.is_offline,
        }
    }

    pub async fn tick(&mut self) {
        let _log = log::get_root_logger();
        let mut name_to_pos: HashMap<String, usize> = HashMap::new();
        loop {
            self.computation_end.wait().await;
            {
                let mut stars_l = self.stars.lock().await;
                let mut iterations = 0;

                if let Some(gwac_rx_chan) = self.gwac_rx_chan.as_mut() { // NOTE online data handling
                    let mut in_frame = false;
                    let mut tot_stars = 0;
                    loop {
                        // TODO handle error logic correctly
                        let data = gwac_rx_chan.recv().await.unwrap();

                        match data {
                            GWACFrame::Start => {
                                in_frame = true;
                            },
                            GWACFrame::End => {
                                break
                            },
                            // NOTE for now do nothing with file name
                            GWACFrame::Filename(filename) => continue,
                            GWACFrame::Star(star) => {
                                if !in_frame {
                                    continue;
                                }

                                if !name_to_pos.contains_key(&star.star_id) {
                                    name_to_pos.insert(star.star_id.clone(), stars_l.len());

                                    let star = Star {
                                        id: star.star_id.clone(),
                                        uid: star.star_id.clone(),
                                        star_type: StarType::Unknown,
                                        model_type: StarModelType::None,
                                        model: parse_model(StarModelType::None, "".to_string(), ),
                                        sample_rate: 15,
                                        samples: None,
                                        samples_tick_index: RefCell::new(0),
                                    };

                                    stars_l.push(
                                        SWStar::new()
                                            .set_star(star)
                                            .set_availables(self.detector_opts.fragment,
                                                            self.detector_opts.skip_delta)
                                            .set_max_buffer_len(100)
                                            .set_window_lens(self.detector_opts.window_length.0 as u32,
                                                             self.detector_opts.window_length.1 as u32)
                                            .build()
                                    );
                                }

                                stars_l[name_to_pos[&star.star_id]].tick(star.mag);
                                tot_stars+=1;
                            },
                        }
                    }

                    //println!("total stars this read: {}", tot_stars);
                } else { // NOTE offline data handling
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
            }
            self.tick_end.wait().await;
        }
    }
}
