use std::collections::{HashSet, HashMap};

// NOTE allows for expansion to include detected (guess) type
//      - flare, microlensing, etc. (positive, maybe positive, etc.)
pub struct DetectorResult {}

pub trait DetectorTrigger {
    fn detect(&mut self, star: &str, val: f32, curren_time: usize, threshold: f32)
              -> Option<DetectorResult>;
}

#[allow(unused)]
pub struct NoneTrigger {}

impl DetectorTrigger for NoneTrigger {
    fn detect(&mut self, _star: &str, _val: f32, _current_time: usize, _threshold: f32)
              -> Option<DetectorResult> {
        None
    }
}

/// Implements a simple threshold trigger
/// mechanism that after the first successful trigger
/// locks and does not trigger again.
///
/// NOTE: This is probably slower than the other method which
///       removes the star from consideration (does not filter it anymore)
///       however in the live filtering case, we will set our values to the
///       worst case of having to filter every star every 10 seconds so these
///       kind of incremental speeds only matter for offline testing.
/// NOTE: Their still is an additional speed penalty in using a hashset each time.
pub struct ThresholdTrigger {
    already_detected_stars: HashSet<String>,
}

impl ThresholdTrigger {
    pub fn new() -> ThresholdTrigger {
        ThresholdTrigger{
            already_detected_stars: HashSet::new()
        }
    }
}

impl DetectorTrigger for ThresholdTrigger {
    fn detect(&mut self, star: &str, val: f32, _current_time: usize, threshold: f32)
              -> Option<DetectorResult> {
        if self.already_detected_stars.contains(star) {
            return None
        }

        if val > threshold {
            self.already_detected_stars.insert(star.to_string());
            Some(DetectorResult{})
        } else {
            None
        }
    }
}

struct ThreeInARowEntry {
    last_time: usize,
    count: usize,
}

pub struct ThreeInARowTrigger {
    delta_skip: usize,
    considered_stars: HashMap<String, ThreeInARowEntry>,
    already_detected_stars: HashSet<String>,
}

impl ThreeInARowTrigger {
    pub fn new(delta_skip: usize) -> ThreeInARowTrigger {
        ThreeInARowTrigger{
            delta_skip,
            considered_stars: HashMap::new(),
            already_detected_stars: HashSet::new(),
        }
    }
}

impl DetectorTrigger for ThreeInARowTrigger {
    fn detect(&mut self, star: &str, val: f32, current_time: usize, threshold: f32)
              -> Option<DetectorResult> {
        if self.already_detected_stars.contains(star) {
            return None
        }

        if val > threshold {
            match self.considered_stars.get_mut(star) {
                Some(star_entry) => {
                    // TODO verify this logic
                    if current_time - star_entry.last_time < self.delta_skip &&
                        star_entry.count < 3 {
                        star_entry.last_time = current_time;
                        star_entry.count += 1;
                        None
                    } else if current_time - star_entry.last_time < self.delta_skip + 1 {
                        self.already_detected_stars.insert(star.to_string());
                        Some(DetectorResult{})
                    } else {
                        star_entry.last_time = current_time;
                        star_entry.count = 1;
                        None
                    }
                }
                None => {
                    self.considered_stars.insert(
                        star.to_string(),
                        ThreeInARowEntry{
                            last_time: current_time,
                            count: 1,
                        });
                    None
                }
            }
        } else {
            None
        }
    }
}
