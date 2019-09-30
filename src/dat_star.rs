/*
 * data format
 * space separated data
 * (time, f(t), tE, start_col, end_col)
 * unsure what everything is but should only need second column
 * -- assumption that everything is sampled at 15 seconds
 * --- For reading GWAC gen data
 */

use crate::star::{parse_model, Star, StarModelType, StarType};
use std::fs;

pub fn parse_star_file(star_file: &str) -> Star {
    let contents =
        fs::read_to_string(&star_file).expect("Failed to read Star DAT file");
    let star_data: Vec<f32> = contents
        .lines()
        .map(|line| {
            -1.0 * line.split_ascii_whitespace().take(2).collect::<Vec<&str>>()[1]
                .parse::<f32>()
                .expect("Failed to parse f(t) data")
        })
        .collect();

    //crate::utils::debug_plt(&star_data, &star_file.to_string(), None);

    Star {
        id: star_file.to_string(),
        uid: star_file.to_string(),
        samples: Some(star_data),
        samples_tick_index: std::cell::RefCell::new(0),
        star_type: StarType::Unknown,
        model_type: StarModelType::None,
        model: parse_model(StarModelType::None, "".to_string()),
        sample_rate: 15,
    }
}
