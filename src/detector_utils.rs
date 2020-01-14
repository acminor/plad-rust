use std::collections::{HashSet, HashMap};
use crate::cyclic_queue::{CyclicQueue, CyclicQueueInterface};

arg_enum! {
    #[derive(Clone, Copy)]
    pub enum DetectorTriggerImps {
        NoneTrigger,
        ThresholdTrigger,
        ThreeInARowTrigger,
    }
}

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
    fn detect(&mut self, _star: &str, _vals: f32, _current_time: usize, _threshold: f32)
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

pub struct ThreeInARowTrigger {
    already_detected_stars: HashSet<String>,
    star_data_windows: HashMap<String, CyclicQueue<f32>>,
}

impl ThreeInARowTrigger {
    pub fn new() -> ThreeInARowTrigger {
        ThreeInARowTrigger{
            already_detected_stars: HashSet::new(),
            star_data_windows: HashMap::new(),
        }
    }
}

impl DetectorTrigger for ThreeInARowTrigger {
    fn detect(&mut self, star: &str, val: f32, _current_time: usize, threshold: f32)
              -> Option<DetectorResult> {
        if self.already_detected_stars.contains(star) {
            return None
        }

        if !self.star_data_windows.contains_key(star) {
            self.star_data_windows.insert(star.to_string(), CyclicQueue::new(3));
        }

        self.star_data_windows.get_mut(star)
            .expect("Star must exist at this point.")
            .push(val);

        if val > threshold && self.star_data_windows[star].len() == 3 {
            let mut is_good = true;
            // NOTE last three elements
            for i in 0..3 {
                is_good &= *self.star_data_windows[star]
                    .get_relative(i)
                    .expect("length check and range should make this impossible") > threshold;
            }

            if is_good {
                self.already_detected_stars.insert(star.to_string());
                Some(DetectorResult{})
            } else {
                None
            }
        } else {
            None
        }
    }
}

// TODO write a trigger that observes curve as outputted to get better result
