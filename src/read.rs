use std::io::{BufRead};

impl<R: BufRead> IsOpen for R {}

pub trait IsOpen: BufRead {
    /// Check if the read stream is still open
    fn is_open(&mut self) -> bool {
        !self.fill_buf().unwrap().is_empty()
    }
}