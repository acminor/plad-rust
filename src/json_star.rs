use crate::star::{parse_model, Star, StarModelType, StarType};
use std::collections::HashMap;
use std::fs;

/*
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
struct OuterFmt {
    currentStarId: Vec<StarDataPoint>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
struct StarDataPoint {
    mag: String,
    sigma_ext_median: String,
    sigma_base: String,
    sigma_ext: String,
    star_id: String,
    magnorm: String,
    ra: String,
    dec: String,
    abSignal: String,
}
*/

pub fn parse_star_file(star_file: &str) -> Option<Star> {
    let contents = match fs::read_to_string(&star_file) {
        Ok(con) => con,
        Err(_) => return None,
    };

    let data: serde_json::Value = match serde_json::from_str(&contents[..]) {
        Ok(data) => data,
        Err(_) => return None,
    };

    let data = if data.is_object() {
        match data.get("currentStarId") {
            Some(_) => &data["currentStarId"],
            None => match data
                .as_object()
                .expect("Problem could not convert JSON data to object.")
                .values()
                .collect::<Vec<&serde_json::Value>>()
                .get(0)
            {
                Some(val) => val,
                None => return None,
            },
        }
    } else {
        &data
    };

    let mut stars = data
        .as_array()
        .unwrap_or_else(|| panic!("Malformed JSON file: {}", star_file))
        .iter()
        .map(|star_dp| {
            (
                star_dp["star_id"]
                    .as_str()
                    .expect("Failed to parse name data"),
                star_dp["magnorm"]
                    .as_str()
                    .expect("Failed to read f(t) data")
                    .parse::<f32>()
                    .expect("Failed to parse f(t) data"),
            )
        })
        .fold(HashMap::new(), |mut map: HashMap<&str, Vec<f32>>, star| {
            match map.get_mut(&star.0) {
                Some(list) => list.push(star.1),
                None => {
                    let mut list = Vec::new();
                    list.push(star.1);
                    map.insert(star.0, list);
                }
            }

            map
        })
        .into_iter()
        .map(|(key, data)| Star {
            id: key.to_string(),
            uid: key.to_string(),
            samples: Some(data),
            samples_tick_index: std::cell::RefCell::new(0),
            star_type: StarType::Unknown,
            model_type: StarModelType::None,
            model: parse_model(StarModelType::None, "".to_string()),
            sample_rate: 15,
        })
        .collect::<Vec<Star>>();

    assert_ne!(stars.len(), 0);
    // NOTE for now assume each file only has one star, code can handle more though
    let star = stars.pop().expect("Problem JSON file contained no stars.");
    //crate::utils::debug_plt(&star.samples[..], &star.uid[..], None);
    Some(star)
}
