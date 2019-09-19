struct Detector;

impl Detector {
    async fn run() {
        loop {
            tick_barrier_main.wait().await;
            match *sd_rx.get_ref(){
                true => {
                    info!(log, "Received finished signal...");
                    break;
                }
                _ => (),
            }

            let (windows, window_names) = {
                let stars = stars.lock().await;

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
            // NOTE signals can modify stars because now only
            //      working with copied data and not refs
            comp_barrier_main.wait().await;

            sample_time += 1;
            iterations += 1;

            let ip = inner_product(
                &templates.templates[..],
                &windows,
                noise_stddev,
                true,
                200,
                200,
            );

            let mut detected_stars = std::collections::HashSet::new();
            ip.iter().zip(window_names).for_each(|(val, star)| {
                if *val > alert_threshold {
                    // TODO this should be a command line option
                    if sample_time >= 40320 && sample_time <= 46080 {
                        // Compute ADP if we have the information to in NFD files
                        // NOTE uses formula from NFD paper
                        uid_to_t0_tp(&star).map(|(t0, t_prime)| {
                            let adp = ((sample_time as f32 - t0) / t_prime) * 100.0;
                            adps.push(adp);
                        });
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

                data.get_mut(&star).unwrap().push(*val);
            });

            // taint detected stars
            // for now just remove
            {
                let mut stars = stars.lock().await;

                let mut iters = 0;
                stars
                    .iter()
                    .filter(|sw| detected_stars.contains(&sw.star.uid))
                    .for_each(|sw| {
                        sw.star.samples.as_ref().map(|samps| {
                            iters +=
                                samps.len() - *sw.star.samples_tick_index.borrow();
                        });
                    });

                ic_tx.send(iters).await;

                stars.retain(|sw| !detected_stars.contains(&sw.star.uid));
            }
        }
    }
}
