use crate::star::Star;

struct SWStar {
    pub star: Star,
    buffer: Vec<f32>,
    _max_buffer_len: u32, // for now unused but potential use in prediction, etc.
    // set these equal to get constant window length
    max_window_len: u32,
    min_window_len: u32,
    pub cur_window_len: u32,
    // These are for keeping track of iterations between matched filtering
    // ex. once every X iterations
    available_pos: u32, // starting at X iteration (for initialization)
    available_count: u32, // have X left
    available_delta: u32, // every X
}

impl SWStar {
    pub fn new() -> SWStarBuilder {
        Default::default()
    }
    pub fn is_ready(&self) -> bool {
        self.min_window_len <= self.cur_window_len
            && self.cur_window_len <= self.max_window_len
            && self.available_count == 0
    }
    pub fn window(&mut self) -> Option<&[f32]> {
        if self.is_ready() {
            self.available_count = self.available_delta;

            Some(&self.buffer[..self.cur_window_len as usize])
        } else {
            None
        }
    }
    // pushes new data and advances state variables one time point
    pub fn tick(&mut self, new_data_point: f32) {
        self.buffer.push(new_data_point);
        // FIXME for now use max_window_len as buffer length
        if self.buffer.len() > self.max_window_len as usize {
            self.buffer.remove(0);
        } else {
            self.cur_window_len+=1;
        }

        self.available_count-=1;
    }
    /*
    fn raw_window() -> Vec<f32> {
        &self.buffer[..self.cur_window_len as usize]
    }
    */
}

#[derive(Default)]
struct SWStarBuilder {
    star: Option<Star>,
    // set these equal to get constant window length
    max_window_len: Option<u32>,
    min_window_len: Option<u32>,
    max_buffer_len: Option<u32>, // for now unused but potential use in prediction, etc.
    // These are for keeping track of iterations between matched filtering
    // ex. once every X iterations
    available_pos: Option<u32>, // starting at X iteration (for initialization)
    available_delta: Option<u32>, // every X
}

impl SWStarBuilder {
    pub fn set_star(&mut self, star: Star) {
        self.star = Some(star);
    }
    pub fn set_window_lens(&mut self, min: u32, max: u32) {
        self.min_window_len = Some(min);
        self.max_window_len = Some(max);
    }
    pub fn set_max_buffer_len(&mut self, max: u32) {
        self.max_buffer_len = Some(max);
    }
    pub fn set_availables(&mut self, pos: u32, delta: u32) {
        self.available_pos = Some(pos);
        self.available_delta = Some(delta);
    }
    pub fn build(self) -> SWStar {
        SWStar {
            star: self.star.unwrap(),
            max_window_len: self.max_window_len.unwrap(),
            min_window_len: self.min_window_len.unwrap(),
            cur_window_len: 0,
            buffer: Vec::new(),
            _max_buffer_len: self.max_buffer_len.unwrap(),
            available_pos: self.available_pos.unwrap(),
            available_delta: self.available_delta.unwrap(),
            available_count: self.available_pos.unwrap(),
        }
    }
}
