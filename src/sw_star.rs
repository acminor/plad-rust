use crate::star::Star;
use std::cell::RefCell;

pub struct SWStar {
    pub star: Star,
    buffer: RefCell<Vec<f32>>,
    _max_buffer_len: u32, // for now unused but potential use in prediction, etc.
    // set these equal to get constant window length
    max_window_len: u32,
    min_window_len: u32,
    pub cur_window_len: RefCell<u32>,
    // These are for keeping track of iterations between matched filtering
    // ex. once every X iterations
    //available_pos: u32, // starting at X iteration (for initialization)
    available_count: RefCell<u32>, // have X left
    available_delta: u32,          // every X
}

impl SWStar {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> SWStarBuilder {
        Default::default()
    }
    pub fn is_ready(&self) -> bool {
        let cur_window_len = *self.cur_window_len.borrow();
        let available_count = *self.available_count.borrow();

        self.min_window_len <= cur_window_len
            && cur_window_len <= self.max_window_len
            && available_count == 0
    }
    // TODO look into ways to prevent this copy
    pub fn window(&self) -> Option<Vec<f32>> {
        let cur_window_len = *self.cur_window_len.borrow();
        let buff = self.buffer.borrow();

        if self.is_ready() {
            self.available_count.replace(self.available_delta);

            Some(buff[..cur_window_len as usize].to_vec())
        } else {
            None
        }
    }
    // pushes new data and advances state variables one time point
    pub fn tick(&self, new_data_point: f32) {
        let mut buff = self.buffer.borrow_mut();
        let cur_window_len = { *self.cur_window_len.borrow() };

        buff.push(new_data_point);
        // FIXME for now use max_window_len as buffer length
        if buff.len() > self.max_window_len as usize {
            buff.remove(0);
        } else {
            self.cur_window_len.replace(cur_window_len + 1);
        }

        let available_count = { *self.available_count.borrow() };

        if cur_window_len >= self.min_window_len {
            self.available_count.replace(available_count - 1);
        }
    }
    /*
    fn raw_window() -> Vec<f32> {
        &self.buffer[..self.cur_window_len as usize]
    }
    */
}

#[derive(Default)]
pub struct SWStarBuilder {
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
    pub fn set_star(mut self, star: Star) -> SWStarBuilder {
        self.star = Some(star);
        self
    }
    pub fn set_window_lens(mut self, min: u32, max: u32) -> SWStarBuilder {
        self.min_window_len = Some(min);
        self.max_window_len = Some(max);
        self
    }
    pub fn set_max_buffer_len(mut self, max: u32) -> SWStarBuilder {
        self.max_buffer_len = Some(max);
        self
    }
    pub fn set_availables(mut self, pos: u32, delta: u32) -> SWStarBuilder {
        self.available_pos = Some(pos);
        self.available_delta = Some(delta);
        self
    }
    pub fn build(self) -> SWStar {
        SWStar {
            star: self.star.expect("Tried to build a partial SWStar."),
            max_window_len: self.max_window_len.expect("Tried to build a partial SWStar."),
            min_window_len: self.min_window_len.expect("Tried to build a partial SWStar."),
            cur_window_len: RefCell::new(0),
            buffer: RefCell::new(Vec::new()),
            _max_buffer_len: self.max_buffer_len.expect("Tried to build a partial SWStar."),
            //available_pos: self.available_pos.unwrap(),
            available_delta: self.available_delta.expect("Tried to build a partial SWStar."),
            // ex. let 'o' be the start pos and 'x' not do anything
            //     8 be delta and 8 be min and max window
            // 1) xxxxxxxoxxxxxxxo -- available_pos = 0
            // 2) xxxxxxxxoxxxxxxxo -- available_pos = 1
            available_count: RefCell::new(self.available_pos.expect("Tried to build a partial SWStar.") + 1),
        }
    }
}
