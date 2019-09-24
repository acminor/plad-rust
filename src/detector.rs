use crate::cli::DetectorOpts;
use crate::sw_star::SWStar;
use crate::async_utils::TwinBarrier;
use crate::info_handler::InformationHandler;
use crate::utils::uid_to_t0_tp;
use crate::utils::inner_product;
use crate::template::Templates;
use crate::log;

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Lock;
use colored::*;

pub struct Detector {
    tick_barrier: TwinBarrier,
    computation_barrier: TwinBarrier,
    info_handler: Arc<InformationHandler>,
    stars: Lock<Vec<SWStar>>,
    templates: Templates,
    detector_opts: DetectorOpts,
}

impl Detector {
    pub async fn run(&mut self) -> (
        HashMap<String, Vec<f32>>,
        HashMap<String, Vec<f32>>,
        Vec<f32>,
        usize,
        usize,
    ) {
        let sd_rx = self.info_handler.get_shutdown_receiver();
        let mut ic_tx = self.info_handler.get_iterations_sender();
        let log = log::get_root_logger();
        let mut sample_time = 0;
        let mut true_events = 0;
        let mut false_events = 0;

        let mut data: HashMap<String, Vec<f32>> = HashMap::new();
        let mut data2: HashMap<String, Vec<f32>> = HashMap::new();
        let mut adps: Vec<f32> = Vec::new();

        {
            let stars = self.stars.lock().await;
            stars.iter().for_each(|sw| {
                if let Some(samps) = sw.star.samples.as_ref() {
                    data2.insert(sw.star.uid.clone(), samps.clone());
                };
            });
        }

        self.computation_barrier.wait().await;
        loop {
            // NOTE check for shutdown before locking
            if *sd_rx.get_ref(){
                info!(log, "Received finished signal...");
                return (data, data2, adps, true_events, false_events);
            }
            self.tick_barrier.wait().await;

            let (windows, window_names) = {
                let stars = self.stars.lock().await;

                let window_names = stars
                    .iter()
                    .filter_map(|sw| {
                        if sw.is_ready() {
                            Some(sw.star.uid.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<String>>();

                let windows = stars
                    .iter()
                    .filter_map(|sw| sw.window())
                    .collect::<Vec<Vec<f32>>>();

                (windows, window_names)
            };

            /*
            if windows.is_empty() {
                println!("empty");
            } else {
                println!("has data");
            }
            */

            // NOTE check for shutdown before locking
            if *sd_rx.get_ref(){
                info!(log, "Received finished signal...");
                return (data, data2, adps, true_events, false_events);
            }
            // NOTE signals can modify stars because now only
            //      working with copied data and not refs
            self.computation_barrier.wait().await;

            sample_time += 1;

            let ip = inner_product(
                &self.templates.templates[..],
                &windows,
                self.detector_opts.noise_stddev,
                true,
                200,
                200,
            );

            let mut detected_stars = std::collections::HashSet::new();
            ip.iter().zip(window_names).for_each(|(val, star)| {
                if *val > self.detector_opts.alert_threshold {
                    // TODO this should be a command line option
                    if sample_time >= 40320 && sample_time <= 46080 {
                        // Compute ADP if we have the information to in NFD files
                        // NOTE uses formula from NFD paper
                        if let Some((t0, t_prime)) = uid_to_t0_tp(&star) {
                            let adp = ((sample_time as f32 - t0) / t_prime) * 100.0;
                            adps.push(adp);
                        };
                        crit!(log, "{}", "TRUE EVENT DETECTED".on_blue();
                              "time"=>sample_time.to_string(),
                              "star"=>star.to_string(),
                              "val"=>val.to_string(),
                        );
                        true_events += 1;
                    } else {
                        crit!(log, "{}", "FALSE EVENT DETECTED".on_red();
                              "time"=>sample_time.to_string(),
                              "star"=>star.to_string(),
                              "val"=>val.to_string(),
                        );
                        false_events += 1;
                    }

                    detected_stars.insert(star.clone());
                }

                if !data.contains_key(&star) {
                    data.insert(star.clone(), Vec::new());
                }

                data.get_mut(&star).expect("Star should be in inner_product data map.").push(*val);
            });

            // taint detected stars
            // for now just remove
            {
                let mut stars = self.stars.lock().await;

                let mut iters = 0;
                stars
                    .iter()
                    .filter(|sw| detected_stars.contains(&sw.star.uid))
                    .for_each(|sw| {
                        if let Some(samps) = sw.star.samples.as_ref() {
                            iters +=
                                samps.len() - *sw.star.samples_tick_index.borrow();
                        };
                    });

                match ic_tx.send(iters).await {
                    _ => () // NOTE for now ignore error b/c non-essential
                };

                stars.retain(|sw| !detected_stars.contains(&sw.star.uid));
            }
        }
    }

    pub fn new(tick_barrier: TwinBarrier, computation_barrier: TwinBarrier,
               info_handler: Arc<InformationHandler>, stars: Lock<Vec<SWStar>>,
               templates: Templates, detector_opts: DetectorOpts) -> Detector {
        Detector {
            tick_barrier,
            computation_barrier,
            info_handler,
            stars,
            templates,
            detector_opts,
        }
    }
}
