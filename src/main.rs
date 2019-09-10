#![feature(proc_macro_hygiene)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_async;

extern crate jemallocator;
extern crate arrayfire;

// [ ] TODO Test if this speeds up the program: also what about memory pressure
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

mod star;
mod template;
mod utils;
mod python;
mod dat_star;
mod cli;
mod log;

use star::*;
use utils::*;
use cli::*;
use log::*;

use arrayfire as AF;

use colored::*;

use std::collections::HashMap;

use cpuprofiler::PROFILER;

static PROF: bool = false;
fn main() {
    if PROF {
        PROFILER
            .lock()
            .unwrap()
            .start("./prof.profile")
            .expect("Couldn't start");
    }

    let log = get_root_logger();
    let run_info = parse_args();

    AF::info();
    AF::set_backend(AF::Backend::CUDA);
    //AF::set_backend(AF::Backend::OPENCL);
    AF::set_device(0);

    let RunInfo {
        stars,
        templates,
        // [ ] TODO see earlier fixme
        _rho,
        noise_stddev,
        window_length,
        alert_threshold,
    } = run_info;

    let templates = templates;
    let mut stars = stars;

    let now = std::time::Instant::now();
    let mut log_timer = std::time::Instant::now();
    /* NOTE: This is a per star, per window counting variable */
    let mut iterations = 0;

    let tot_stars = stars.len();
    let max_len: usize
        = stars.iter().map(|star| star.samples.len()).max().unwrap();
    let tot_iter: usize =
        stars.iter().map(|star| star.samples.len()).sum::<usize>()
        / window_length as usize;

    info!(
        log, "";
        "window_length"=>format!("{}", window_length),
        "total_iters_needed"=>tot_iter,
    );

    let is_offline = true;
    let mut i = 0;
    let mut sample_time = 0;
    let mut true_events = 0;
    let mut false_events = 0;
    let mut data: HashMap<String, Vec<f32>> = HashMap::new();
    stars.iter()
        .for_each(|star| {
            data.insert(star.uid.clone(), Vec::new());
        });
    loop {
        if log_timer.elapsed() > std::time::Duration::from_secs(2) {
            let sps = iterations as f32 / now.elapsed().as_secs() as f32;
            let pp = (iterations as f32)/(tot_iter as f32) * 100.0;
            info!(log, "";
                  "TotTime"=>format!("{}s", now.elapsed().as_secs()),
                  "IterationsLeft"=>format!("{}", tot_iter - iterations as usize),
                  "EstTimeLeft"=>format!("{}s", (tot_iter - iterations as usize)
                                         as f32/sps as f32),
                  "StarsPerSec"=>format!("{}", sps),
                  "StarsPerTenSec"=>format!("{}", sps*10.0),
                  "%Progress"=>format!("{}%", pp));

            log_timer = std::time::Instant::now();
        }
        let mut cur_stars = stars
            .iter_mut()
            .filter(|star| star.samples.len() >= (window_length as usize))
            .collect::<Vec<&mut Star>>();

        if cur_stars.is_empty() && is_offline {
            break;
        }

        if i == -1 {
            break;
        } else {
            i+=1;
        }

        let windows = cur_stars
            .iter_mut()
            .map(|star| {
                iterations += 1;

                star.samples.drain(0..(window_length as usize)).collect()
            })
            .collect::<Vec<Vec<f32>>>();
        sample_time += window_length;

        let window_names = cur_stars
            .iter()
            .map(|star| {
                star.uid.clone()
            })
            .collect::<Vec<String>>();

        let ip = inner_product(
            &templates.templates[..],
            &windows[..],
            window_length as usize,
            noise_stddev,
            true,
            200,
            200,
        );

        let mut detected_stars = std::collections::HashSet::new();
        ip
            .iter()
            .zip(window_names)
            .for_each(|(val, star)| {
                if *val > alert_threshold {
                    // TODO this should be a command line option
                    if sample_time >= 40320 && sample_time <= 46080 {
                        crit!(log, "{}", "TRUE EVENT DETECTED".on_blue();
                              "time"=>sample_time.to_string(),
                              "star"=>star.to_string(),
                        );
                        true_events += 1;
                    } else {
                        crit!(log, "{}", "FALSE EVENT DETECTED".on_red();
                              "time"=>sample_time.to_string(),
                              "star"=>star.to_string(),
                        );
                        false_events += 1;
                    }

                    detected_stars.insert(star.clone());
                }

                data.get_mut(&star).unwrap().push(*val);
            });

        // taint detected stars
        // for now just remove
        stars = stars
            .into_iter()
            .filter(|star| !detected_stars.contains(&star.uid))
            .collect();
    }

    compute_and_disp_stats(&data);

    info!(log, "{}", "Run Stats".on_green();
          "num_events_detected"=>true_events+false_events,
          "num_true_events"=>true_events,
          "num_false_events"=>false_events,
          "num_stars"=>tot_stars,
          "max_star_len"=>max_len);

    for (star_title, star_data) in data.iter() {
        crate::utils::debug_plt(&star_data, star_title, None);
    }

    if PROF {
        PROFILER.lock().unwrap().stop().expect("Couldn't start");
    }
}

fn compute_and_disp_stats(data: &HashMap<String, Vec<f32>>) {
    let log = get_root_logger();

    let stats = |data: &Vec<f32>| {
        let mut avg = 0.0;
        let mut min = std::f32::INFINITY;
        let mut max = std::f32::NEG_INFINITY;
        let mut std_dev = 0.0;
        let len = data.len() as f32;

        for &datum in data {
            avg += datum;
            min = if min < datum {
                min
            } else {
                datum
            };
            max = if max > datum {
                max
            } else {
                datum
            };
            std_dev += datum*datum;
        }

        avg /= len;
        std_dev = (std_dev/len - avg*avg).sqrt();

        (min, max, avg, std_dev)
    };

    { // over all values
        let (min, max, avg, std_dev) =
            stats(&data
                  .iter()
                  .flat_map(|(_key, val)| {
                      val.clone()
                  }).collect());
        info!(log, "{}", "All values stats:".on_blue();
              "min"=>min.to_string(),
              "max"=>max.to_string(),
              "avg"=>avg.to_string(),
              "std_dev"=>std_dev.to_string());
    }

    {
        let ch_sz = 500;
        let mut group_stats = Vec::new();
        for (_key, star) in data.iter() {
            for (i, chunk) in star.chunks(ch_sz).enumerate() {
                if group_stats.len() <= i {
                    let temp = chunk.to_vec();
                    group_stats.push(temp);
                } else {
                    let mut temp = chunk.to_vec();
                    group_stats[i].append(&mut temp);
                }
            }
        }

        group_stats.iter().enumerate().for_each(|(i, group)| {
            let (min, max, avg, std_dev) = stats(&group);
            info!(log, "{}", format!("Group {} values stats:", i).on_green();
                  "min"=>min.to_string(),
                  "max"=>max.to_string(),
                  "avg"=>avg.to_string(),
                  "std_dev"=>std_dev.to_string());
        })
    }

    { // TODO comment ???
        let star_stats = || {
            data
                .iter()
                .map(|(_key, val)| {
                    stats(val)
                })
        };

        let mins = star_stats()
            .map(|tup| {tup.0})
            .collect::<Vec<f32>>();
        let maxs = star_stats()
            .map(|tup| {tup.1})
            .collect::<Vec<f32>>();
        let avgs = star_stats()
            .map(|tup| {tup.2})
            .collect::<Vec<f32>>();

        let (min, max, avg, std_dev) = stats(&mins);
        info!(log, "{}", "Min values stats: ".on_red();
              "min"=>min.to_string(),
              "max"=>max.to_string(),
              "avg"=>avg.to_string(),
              "std_dev"=>std_dev.to_string());

        let (min, max, avg, std_dev) = stats(&maxs);
        info!(log, "{}", "Max values stats: ".on_red();
              "min"=>min.to_string(),
              "max"=>max.to_string(),
              "avg"=>avg.to_string(),
              "std_dev"=>std_dev.to_string());

        let (min, max, avg, std_dev) = stats(&avgs);
        info!(log, "{}", "Avg values stats: ".on_red();
              "min"=>min.to_string(),
              "max"=>max.to_string(),
              "avg"=>avg.to_string(),
              "std_dev"=>std_dev.to_string());
    }
}
