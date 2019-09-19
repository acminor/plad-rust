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

use cli::*;
use log::*;
use sw_star::*;
use utils::*;

use arrayfire as AF;

use colored::*;

use std::collections::HashMap;
//use std::sync::mpsc::channel;
use std::sync::{
    //mpsc::{Receiver, Sender, TryRecvError},
    Arc, //Barrier, Mutex,
};

use std::sync::mpsc::channel as sync_channel;
use std::sync::mpsc::Receiver as SyncReceiver;
use std::sync::mpsc::Sender as SyncSender;

use tokio::sync::{
    mpsc::{Receiver, Sender, error::{RecvError}, channel},
    Lock,
};

use cpuprofiler::PROFILER;

struct Barrier {
    tx_go: Sender<bool>,
    rx_go: Receiver<bool>,
}

impl Barrier {
    async fn tx_go(&mut self) {
        self.tx_go.send(true).await;
    }

    async fn rx_go(&mut self) {
        self.rx_go.recv().await;
    }
}

struct RunState {
    total_iterations: usize,
    is_offline: bool,
    stars: Lock<Vec<SWStar>>,
    computation_end: Barrier,
    tick_end: Barrier,
    iterations_chan: (Sender<usize>, Receiver<usize>),
    shutdown_chan: SyncSender<bool>,
    // FIXME average stars per fragment
    // FIXME average stars per iteration???
}

// computation_end is a misnomer, it merely means that all the data for the current window
//   has been copied and thus stars can be safely updated without producing unintended results
// tick_end signifies that main can start the next computation
fn tick_driver(state: RunState) {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    let RunState {
        total_iterations,
        is_offline,
        mut stars,
        iterations_chan,
        mut computation_end,
        mut tick_end,
        mut shutdown_chan,
    } = state;

    let (mut iterations_chan_tx, mut iterations_chan_rx) = iterations_chan;
    rt.block_on(async move {
        tokio::spawn(async move {
            let log = log::get_root_logger();
            let mut iterations = 0;
            let mut log_timer = std::time::Instant::now();
            let now = std::time::Instant::now();
            loop {
                // FIXME do error handling with shutdown chan here at some point
                match iterations_chan_rx.recv().await {
                    Some(val) => iterations += val,
                    _ => (),
                }

                if iterations == total_iterations && is_offline {
                    info!(log, "Sending shutdown signal...");
                    shutdown_chan.send(true);
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
        });

        let log = log::get_root_logger();
        loop {
            // NOTE this will serve as explanation of other barriers
            // -- order is really important
            //
            // 1) block waiting for main to send
            // 2) main blocks waiting for tick to send
            // 3) tick sends after unblock, freeing main
            // -- thus we have a barrier
            //
            // oxo---
            // \/     o = send, x = block, - = unblocked
            // xo----
            computation_end.rx_go().await;
            computation_end.tx_go().await;
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
            tick_end.rx_go().await;
            tick_end.tx_go().await;
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

    /*
    let computation_end = Arc::new(Barrier::new(2));
    let tick_end = Arc::new(Barrier::new(2));
    */

    let computation_end_main = channel(10);
    let computation_end_tick = channel(10);
    let tick_end_main = channel(10);
    let tick_end_tick = channel(10);

    let computation_barrier_tick = Barrier {
        tx_go: computation_end_tick.0,
        rx_go: computation_end_main.1,
    };
    let mut computation_barrier_main = Barrier {
        tx_go: computation_end_main.0,
        rx_go: computation_end_tick.1,
    };

    let tick_barrier_tick = Barrier {
        tx_go: tick_end_tick.0,
        rx_go: tick_end_main.1,
    };
    let mut tick_barrier_main = Barrier {
        tx_go: tick_end_main.0,
        rx_go: tick_end_tick.1,
    };

    let (mut ic_tx, mut ic_rx) = channel(10);
    let (mut sd_tx, mut sd_rx) = sync_channel();
    {
        let stars = stars.clone();
        let ic_tx = ic_tx.clone();
        std::thread::spawn(move || {
            let run_state = RunState {
                stars: stars,
                iterations_chan: (ic_tx, ic_rx),
                shutdown_chan: sd_tx,
                tick_end: tick_barrier_tick,
                computation_end: computation_barrier_tick,
                total_iterations: tot_iter,
                is_offline,
            };

            tick_driver(run_state);
        });
    }

    //computation_end.wait();
    computation_barrier_main.tx_go().await;
    computation_barrier_main.rx_go().await;
    loop {
        //tick_end.wait();
        tick_barrier_main.tx_go().await;
        tick_barrier_main.rx_go().await;
        match sd_rx.try_recv(){
            Ok(val) if val => {
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
        //computation_end.wait();
        computation_barrier_main.tx_go().await;
        computation_barrier_main.rx_go().await;

        sample_time += 1;

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
