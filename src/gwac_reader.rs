use tokio::sync::mpsc::{Sender, Receiver};
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

struct GWACData {
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

enum GWACFrame {
    Start,
    End,
    Star(GWACData)
}

struct GWACReader {
    data_file: tokio::io::BufReader<tokio::fs::File>,
    data_chan: (Sender<GWACFrame>, Arc<Receiver<GWACFrame>>),
}

impl GWACReader {
    pub fn new() -> GWACReader {
        GWACReader {
        }
    }

    pub fn get_data_channel(&self) -> Arc<Receiver<GWACFrame>> {
        self.data_chan.1.clone()
    }

    pub async fn start(&self) {
        let mut buf = String::new();
        loop {
            match self.data_file.read_line(&mut buf).await {
                Ok(val) if val == 0 => break, // TODO graceful shutdown
                Ok(_) => (),
                _ => break, // TODO graceful shutdown
            }

            let data = buf.trim();

            match data {
                "start" => {
                    self.data_chan.0.send(GWACFrame::Start).await;
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
                        }));
                }
            }
        }
    }
}
