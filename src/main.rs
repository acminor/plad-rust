#![feature(proc_macro_hygiene)]

#[cfg(test)]
#[macro_use]
extern crate approx;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate clap;

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

mod async_utils;
mod cli;
mod dat_star;
mod detector;
mod gwac_reader;
mod info_handler;
mod json_star;
mod log;
mod python;
mod star;
mod sw_star;
mod template;
mod ticker;
mod toml_star;
mod utils;
mod filter;
mod filter_utils;
mod tester;

use async_utils::{twin_barrier, TwinBarrier};
use cli::*;
use detector::Detector;
use gwac_reader::GWACReader;
use info_handler::InformationHandler;
use log::*;
use sw_star::*;
use ticker::Ticker;

use arrayfire as AF;

use colored::*;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use tokio::sync::Lock;

use cpuprofiler::PROFILER;

struct RunState {
    stars: Lock<Vec<SWStar>>,
    computation_end: TwinBarrier,
    tick_end: TwinBarrier,
    info_handler: Arc<InformationHandler>,
    detector_opts: DetectorOpts,
    gwac_reader: Option<GWACReader>,
    // FIXME average stars per fragment
    // FIXME average stars per iteration???
}

// computation_end is a misnomer, it merely means that all the data for the current window
//   has been copied and thus stars can be safely updated without producing unintended results
// tick_end signifies that main can start the next computation
fn tick_driver(state: RunState) {
    let rt = tokio::runtime::Builder::new()
        .blocking_threads(4)
        .core_threads(4)
        .build()
        .expect("Problem building tokio runtime.");
    let RunState {
        stars,
        computation_end,
        tick_end,
        info_handler,
        mut gwac_reader,
        detector_opts,
    } = state;

    rt.block_on(async move {
        {
            let info_handler = info_handler.clone();
            tokio::spawn(async move {
                info_handler.progress_log().await;
            });
        }

        let gwac_rx_chan = if gwac_reader.is_some() {
            Some(
                gwac_reader
                    .as_mut()
                    .expect("Should never happen.")
                    .get_data_channel(),
            )
        } else {
            None
        };

        if let Some(mut gwac_reader) = gwac_reader {
            tokio::spawn(async move {
                gwac_reader.start().await;
            });
        }

        Ticker::new(
            computation_end,
            tick_end,
            stars,
            gwac_rx_chan,
            detector_opts.clone(),
            info_handler,
        )
        .tick()
        .await;
    });
}

static PROF: bool = true;
static CC_COUNT: AtomicU8 = AtomicU8::new(0);
static MAIN_SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[tokio::main]
async fn main() {
    if PROF {
        PROFILER
            .lock()
            .expect("Couldn't lock PROFILER")
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
        gwac_reader,
        // [ ] TODO see earlier fixme
        detector_opts,
        log_opts,
        tester,
    } = run_info;

    let mut stars = Lock::new(stars);

    let stars_t = stars.lock().await;
    let tot_stars = stars_t.len();

    let max_len: Option<usize> = stars_t
        .iter()
        .filter_map(|sw| sw.star.samples.as_ref())
        .map(|samps| samps.len())
        .max();

    let tot_iter: Option<usize> = if !gwac_reader.is_some() {
        Some(
            stars_t
                .iter()
                .filter_map(|sw| sw.star.samples.as_ref())
                .map(|samps| samps.len())
                .sum::<usize>(),
        )
    } else {
        None
    };

    drop(stars_t);

    info!(
        log, "";
        "window_length"=>format!("{:?}", detector_opts.window_length),
        "total_iters_needed"=>tot_iter,
    );

    let is_offline = !gwac_reader.is_some();
    let info_handler = Arc::new(InformationHandler::new(is_offline, tot_iter));
    let (tick_barrier_main, tick_barrier_tick) = twin_barrier();
    let (comp_barrier_main, comp_barrier_tick) = twin_barrier();
    {
        let stars = stars.clone();
        let info_handler = info_handler.clone();
        let detector_opts = detector_opts.clone();
        std::thread::spawn(move || {
            let run_state = RunState {
                stars,
                tick_end: tick_barrier_tick,
                computation_end: comp_barrier_tick,
                info_handler,
                gwac_reader,
                detector_opts,
            };

            tick_driver(run_state);
        });
    }

    {
        let info_handler = info_handler.clone();
        // TODO double cc should cause quit
        ctrlc::set_handler(move || {
            if CC_COUNT.load(Ordering::Relaxed) == 0 {
                if PROF {
                    PROFILER
                        .lock()
                        .expect("Couldn't lock profiler.")
                        .stop()
                        .expect("Couldn't start");
                }

                if MAIN_SHUTDOWN.load(Ordering::Relaxed) {
                    std::process::exit(-1);
                }

                info_handler.trigger_shutdown();
                CC_COUNT.store(1, Ordering::Relaxed);
            } else {
                std::process::exit(-1);
            }
        })
        .expect("Issue setting Ctrl-C handler.");
    }

    let mut detector = {
        let stars = stars.clone();
        //let into_handler = info_handler.clone();
        let detector_opts = detector_opts.clone();

        Detector::new(
            tick_barrier_main,
            comp_barrier_main,
            info_handler,
            stars,
            templates,
            tester,
            detector_opts,
        )
    };

    let (data, data2, adps, true_events, false_events) = detector.run().await;

    // so ctrl-c handler knows to shutdown on first or second ctrl-c
    MAIN_SHUTDOWN.store(true, Ordering::Relaxed);

    compute_and_disp_stats(&data, &adps[..]);

    info!(log, "{}", "Run Stats".on_green();
          "num_events_detected"=>true_events+false_events,
          "num_true_events"=>true_events,
          "num_false_events"=>false_events,
          "num_stars"=>tot_stars,
          "max_star_len"=>max_len);

    let mut data = data.iter().collect::<Vec<(&String, &Vec<f32>)>>();

    let sort = |data: &mut Vec<(&String, &Vec<f32>)>| {
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

            max_a
                .partial_cmp(&max_b)
                .expect("Invalid value in data sort.")
        });
    };

    let data = match log_opts.sort {
        SortOpt::None => {
            data
        },
        SortOpt::Increasing => {
            sort(&mut data);
            data
        },
        SortOpt::Decreasing => {
            sort(&mut data);
            data.reverse();
            data
        }
    };

    if log_opts.plot {
        for (star_title, star_data) in data.into_iter() {
            if is_offline {
                //crate::utils::debug_plt(&star_data, star_title, None);
                crate::utils::debug_plt_2(
                    &star_data,
                    data2.get(star_title).expect("Star should be in data2."),
                    star_title,
                    detector_opts.skip_delta,
                );
            } else {
                crate::utils::debug_plt(&star_data, star_title, None);
            }
        }
    }

    if PROF {
        PROFILER
            .lock()
            .expect("Couldn't lock PROFILER.")
            .stop()
            .expect("Couldn't start");
    }
}

fn compute_and_disp_stats(data: &HashMap<String, Vec<f32>>, adps: &[f32]) {
    let log = get_root_logger();

    let stats = |data: &[f32]| {
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
        let (min, max, avg, std_dev) = stats(
            &data
                .iter()
                .flat_map(|(_key, val)| val.clone())
                .collect::<Vec<f32>>()[..],
        );
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
