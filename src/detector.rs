use crate::async_utils::TwinBarrier;
use crate::cli::DetectorOpts;
use crate::info_handler::InformationHandler;
use crate::log;
use crate::sw_star::SWStar;
use crate::template::Templates;
//use crate::utils::inner_product;
use crate::filter::inner_product;
use crate::utils::uid_to_t0_tp;
use crate::tester::Tester;

use colored::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::panic;
use tokio::sync::Lock;

pub struct Detector {
    tick_barrier: TwinBarrier,
    computation_barrier: TwinBarrier,
    info_handler: Arc<InformationHandler>,
    stars: Lock<Vec<SWStar>>,
    templates: Templates,
    tester: Box<dyn Tester>,
    detector_opts: DetectorOpts,
}

impl Detector {
    pub async fn run(
        &mut self,
    ) -> (
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
            if *sd_rx.get_ref() {
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
            if *sd_rx.get_ref() {
                info!(log, "Detector received finished signal...");
                return (data, data2, adps, true_events, false_events);
            }
            // NOTE signals can modify stars because now only
            //      working with copied data and not refs
            self.computation_barrier.wait().await;

            sample_time += 1;

            // NOTE gracefully handle bugs in CUDA and NVIDIA drivers
            //      along with any other bugs in Arrayfire
            // - in testing we had issues so this is here to ignoring
            //   temporary transient bugs
            // - to handle the error we just skip the current iteration
            //   and move on.
            // - NOTE we could also try to retry, but given time constraints
            //   and the fact that the bug may represent it self as some
            //   particular property of the data, we choose to skip this point
            // - NOTE should be fine to AssertUnwindSafe, main compile issues
            //   seem to come from being within an async context.
            //   - The actual inner_product and arguments should be fine.
            /*
            let ip = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                inner_product(
                    &self.templates.templates[..],
                    &windows,
                    &window_names,
                    sample_time,
                    self.detector_opts.noise_stddev,
                    true,
                    self.detector_opts.dc_norm,
                    200,
                    200,
                )
            }));
            */

            let ip: Result<Vec<f32>, usize> = Ok(inner_product(
                &self.templates.templates[..],
                &windows,
                &window_names,
                sample_time,
                self.detector_opts.noise_stddev,
                true,
                self.detector_opts.dc_norm,
                // [ ] FIXME XXX had to increase but wrong logic is affecting results
                // - quick visual test, reduce number and run on constant stars
                //   - very easy to see the jumps and issues
                //   - also could add logging at transition point
                //     to historical mean removal to see other issues
                2000,
                2000,
            ));

            let ip = match ip {
                Ok(ip) => ip,
                _ => continue,
            };

            let mut detected_stars = std::collections::HashSet::new();
            ip.iter().zip(window_names).for_each(|(val, star)| {
                if *val > self.detector_opts.alert_threshold {
                    // compute values b/c tester is a valid tester
                    if self.tester.is_valid() {
                        if self.tester.is_true_positive(&star, sample_time) {
                            adps.push(self.tester.adp(&star, sample_time));
                            crit!(log, "{}", "TRUE EVENT DETECTED".on_blue();
                                  "time"=>sample_time.to_string(),
                                  "star"=>star.to_string(),
                                  "val"=>val.to_string(),
                            );
                            true_events += 1;
                        } else { // NOTE: is_true_pos mutually exclusive of false_pos
                            crit!(log, "{}", "FALSE EVENT DETECTED".on_red();
                                  "time"=>sample_time.to_string(),
                                  "star"=>star.to_string(),
                                  "val"=>val.to_string(),
                            );
                            false_events += 1;
                        }
                    }

                    detected_stars.insert(star.clone());
                }

                if !data.contains_key(&star) {
                    data.insert(star.clone(), Vec::new());
                }

                data.get_mut(&star)
                    .expect("Star should be in inner_product data map.")
                    .push(*val);
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
                            iters += samps.len()
                                - *sw.star.samples_tick_index.borrow();
                        };
                    });

                match ic_tx.send(iters).await {
                    _ => (), // NOTE for now ignore error b/c non-essential
                };

                stars.retain(|sw| !detected_stars.contains(&sw.star.uid));
            }
        }
    }

    pub fn new(
        tick_barrier: TwinBarrier,
        computation_barrier: TwinBarrier,
        info_handler: Arc<InformationHandler>,
        stars: Lock<Vec<SWStar>>,
        templates: Templates,
        tester: Box<dyn Tester>,
        detector_opts: DetectorOpts,
    ) -> Detector {
        Detector {
            tick_barrier,
            computation_barrier,
            info_handler,
            stars,
            templates,
            tester,
            detector_opts,
        }
    }
}
