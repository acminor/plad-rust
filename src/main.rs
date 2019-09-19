#![feature(proc_macro_hygiene)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate tokio;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_term;

extern crate arrayfire;
extern crate jemallocator;

// [ ] TODO Test if this speeds up the program: also what about memory pressure
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

mod cli;
mod dat_star;
mod json_star;
mod log;
mod python;
mod star;
mod sw_star;
mod template;
mod toml_star;
mod utils;
mod async_utils;
mod info_handler;

use cli::*;
use log::*;
use sw_star::*;
use utils::*;
use async_utils::{TwinBarrier, twin_barrier};
use info_handler::InformationHandler;

use arrayfire as AF;

use colored::*;

use std::collections::HashMap;
use std::sync::Arc;

use std::sync::mpsc::channel as sync_channel;
use std::sync::mpsc::Receiver as SyncReceiver;
use std::sync::mpsc::Sender as SyncSender;

use tokio::sync::{
    mpsc::{Receiver, Sender, error::{RecvError}, channel},
    Lock,
};

use cpuprofiler::PROFILER;

struct RunState {
    is_offline: bool,
    stars: Lock<Vec<SWStar>>,
    computation_end: TwinBarrier,
    tick_end: TwinBarrier,
    info_handler: InformationHandler,
    // FIXME average stars per fragment
    // FIXME average stars per iteration???
}

// computation_end is a misnomer, it merely means that all the data for the current window
//   has been copied and thus stars can be safely updated without producing unintended results
// tick_end signifies that main can start the next computation
fn tick_driver(state: RunState) {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    let RunState {
        is_offline,
        mut stars,
        mut computation_end,
        mut tick_end,
        info_handler,
    } = state;

    let mut iterations_chan_tx = info_handler.get_iterations_sender();
    rt.block_on(async move {
        tokio::spawn(async move {
            info_handler.progress_log().await;
        });

        let log = log::get_root_logger();
        loop {
            computation_end.wait().await;
            {
                let stars_l = stars.lock().await;
                let mut iterations = 0;
                stars_l.iter().for_each(|sw| {
                    sw.star.samples.as_ref().map(|samps| {
                        let tick_index = { *sw.star.samples_tick_index.borrow() };

                        if tick_index < samps.len() {
                            sw.tick(samps[tick_index]);
                            iterations += 1;
                            sw.star.samples_tick_index.replace(tick_index + 1);
                        }
                    });
                });
                iterations_chan_tx.send(iterations).await;
            }
            tick_end.wait().await;
        }
    });
}

static PROF: bool = true;

#[tokio::main]
async fn main() {
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
    AF::set_device(0);

    let RunInfo {
        stars,
        templates,
        // [ ] TODO see earlier fixme
        _rho,
        noise_stddev,
        window_length,
        skip_delta,
        fragment,
        alert_threshold,
    } = run_info;

    let stars = stars
        .into_iter()
        .zip((0..fragment).cycle())
        .map(|(star, fragment)| {
            SWStar::new()
                .set_star(star)
                .set_availables(fragment, skip_delta)
                .set_max_buffer_len(100)
                .set_window_lens(window_length.0 as u32, window_length.1 as u32)
                .build()
        })
        .collect::<Vec<SWStar>>();

    let mut stars = Lock::new(stars);
    let templates = templates;

    let stars_t = stars.lock().await;
    let tot_stars = stars_t.len();
    let max_len: usize = stars_t
        .iter()
        .filter_map(|sw| sw.star.samples.as_ref())
        .map(|samps| samps.len())
        .max()
        .unwrap();
    let tot_iter: usize = stars_t
        .iter()
        .filter_map(|sw| sw.star.samples.as_ref())
        .map(|samps| samps.len())
        .sum::<usize>();

    info!(
        log, "";
        "window_length"=>format!("{:?}", window_length),
        "total_iters_needed"=>tot_iter,
    );

    let is_offline = true;
    let mut sample_time = 0;
    let mut true_events = 0;
    let mut false_events = 0;
    let mut data: HashMap<String, Vec<f32>> = HashMap::new();
    let mut data2: HashMap<String, Vec<f32>> = HashMap::new();
    let mut adps: Vec<f32> = Vec::new();
    stars_t.iter().for_each(|sw| {
        data.insert(sw.star.uid.clone(), Vec::new());
        sw.star.samples.as_ref().map(|samps| {
            data2.insert(sw.star.uid.clone(), samps.clone());
        });
    });
    drop(stars_t);

    let (comp_barrier_main, comp_barrier_tick) = twin_barrier();
    let (tick_barrier_main, tick_barrier_tick) = twin_barrier();
    let info_handler = InformationHandler::new(tot_iter);
    let mut ic_tx = info_handler.get_iterations_sender();
    let sd_rx = info_handler.get_shutdown_receiver();
    {
        let stars = stars.clone();
        std::thread::spawn(move || {
            let run_state = RunState {
                stars: stars,
                tick_end: tick_barrier_tick,
                computation_end: comp_barrier_tick,
                is_offline,
                info_handler,
            };

            tick_driver(run_state);
        });
    }

    let mut log_timer = std::time::Instant::now();
    let now = std::time::Instant::now();
    let mut iterations = 0;
    comp_barrier_main.wait().await;
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

    compute_and_disp_stats(&data, &adps);

    info!(log, "{}", "Run Stats".on_green();
          "num_events_detected"=>true_events+false_events,
          "num_true_events"=>true_events,
          "num_false_events"=>false_events,
          "num_stars"=>tot_stars,
          "max_star_len"=>max_len);

    let mut data = data.iter().collect::<Vec<(&String, &Vec<f32>)>>();
    data.sort_unstable_by(|a, b| {
        let max_a = {
            let mut temp_max = -1.0f32;
            for &i in a.1.iter() {
                if i > temp_max {
                    temp_max = i;
                }
            }

            temp_max
        };

        let max_b = {
            let mut temp_max = -1.0f32;
            for &i in b.1.iter() {
                if i > temp_max {
                    temp_max = i;
                }
            }

            temp_max
        };

        max_a.partial_cmp(&max_b).unwrap()
    });
    data.reverse();
    for (star_title, star_data) in data.into_iter() {
        //crate::utils::debug_plt(&star_data, star_title, None);
        crate::utils::debug_plt_2(
            &star_data,
            data2.get(star_title).unwrap(),
            star_title,
            skip_delta,
        );
    }

    if PROF {
        PROFILER.lock().unwrap().stop().expect("Couldn't start");
    }
}

fn compute_and_disp_stats(data: &HashMap<String, Vec<f32>>, adps: &Vec<f32>) {
    let log = get_root_logger();

    let stats = |data: &Vec<f32>| {
        let mut avg = 0.0;
        let mut min = std::f32::INFINITY;
        let mut max = std::f32::NEG_INFINITY;
        let mut std_dev = 0.0;
        let len = data.len() as f32;

        for &datum in data {
            avg += datum;
            min = if min < datum { min } else { datum };
            max = if max > datum { max } else { datum };
            std_dev += datum * datum;
        }

        avg /= len;
        std_dev = (std_dev / len - avg * avg).sqrt();

        (min, max, avg, std_dev)
    };

    {
        let (min, max, avg, std_dev) = stats(&adps);
        info!(log, "{}", "ADP stats:".on_blue();
              "min"=>min.to_string(),
              "max"=>max.to_string(),
              "avg"=>avg.to_string(),
              "std_dev"=>std_dev.to_string());
    }

    {
        // over all values
        let (min, max, avg, std_dev) =
            stats(&data.iter().flat_map(|(_key, val)| val.clone()).collect());
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

    {
        // TODO comment ???
        let star_stats = || data.iter().map(|(_key, val)| stats(val));

        let mins = star_stats().map(|tup| tup.0).collect::<Vec<f32>>();
        let maxs = star_stats().map(|tup| tup.1).collect::<Vec<f32>>();
        let avgs = star_stats().map(|tup| tup.2).collect::<Vec<f32>>();

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
