use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio::io::BufReader;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use std::sync::Arc;

macro_rules! unwrap_or_continue {
    ($e:expr) => {
        match $e {
            Ok(val) => val,
            _ => continue,
        }
    };
}

pub struct GWACData {
    xpix: f32,
    ypix: f32,
    ra: f32,
    dec: f32,
    zone: String,
    star_id: String,
    mag: f32,
    timestamp: f32,
    ellipiticity: f32,
    ccd_num: String,
}

pub enum GWACFrame {
    Start,
    Filename(String),
    End,
    Star(GWACData),
}

pub struct GWACReader {
    // NOTE lazily initialize so that new function is non-async
    data_file_path: String,
    // NOTE use option so later can move out of it
    data_chan: (Sender<GWACFrame>, Option<Receiver<GWACFrame>>),
}

impl GWACReader {
    pub fn new(data_file: &str) -> GWACReader {

        let (tx, rx) = channel(64);
        let data_chan = (tx, Some(rx));

        GWACReader {
            data_file_path: data_file.to_string(),
            data_chan,
        }
    }

    pub fn get_data_channel(&mut self) -> Receiver<GWACFrame> {
        if self.data_chan.1.is_some() {
            self.data_chan.1.take().unwrap()
        } else {
            panic!("Only one GWAC data receiver is allowed.")
        }
    }

    // NOTE should only be called once
    pub async fn start(&mut self) {
        let mut data_file = File::open(&self.data_file_path).await.expect("Could not open GWAC file.");
        let mut data_file = BufReader::new(data_file);

        let mut buf = String::new();
        let mut recently_started = false;
        loop {
            match data_file.read_line(&mut buf).await {
                Ok(val) if val == 0 => break, // TODO graceful shutdown
                Ok(_) => (),
                _ => break, // TODO graceful shutdown
            }

            let data = buf.trim();

            // NOTE: Right after start signal a file name is sent
            //       this logic handles parsing and sending that
            if recently_started {
                self.data_chan.0.send(GWACFrame::Filename(data.to_string())).await;
                recently_started = false;
                continue;
            }

            match data {
                "start" => {
                    self.data_chan.0.send(GWACFrame::Start).await;
                    recently_started = true;
                },
                "end" => {
                    self.data_chan.0.send(GWACFrame::Start).await;
                },
                val => {
                    let fields = val.split_whitespace().collect::<Vec<&str>>();

                    let xpix = unwrap_or_continue!(fields[0].parse::<f32>());
                    let ypix = unwrap_or_continue!(fields[1].parse::<f32>());
                    let ra = unwrap_or_continue!(fields[2].parse::<f32>());
                    let dec = unwrap_or_continue!(fields[3].parse::<f32>());
                    let zone = fields[4].trim().to_string();
                    let star_id = fields[5].trim().to_string();
                    let mag = unwrap_or_continue!(fields[6].parse::<f32>());
                    let timestamp = unwrap_or_continue!(fields[7].parse::<f32>());
                    let ellipiticity = unwrap_or_continue!(fields[8].parse::<f32>());
                    let ccd_num = fields[9].trim().to_string();

                    self.data_chan.0.send(GWACFrame::Star(
                        GWACData {
                            xpix, ypix, ra, dec, zone, star_id, mag, timestamp, ellipiticity, ccd_num,
                        })).await;
                }
            }
        }
    }
}
