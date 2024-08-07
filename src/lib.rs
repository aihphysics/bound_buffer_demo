use std::collections::vec_deque::VecDeque;
use std::io::{stdout, Write};
use std::sync::{Arc, Condvar, Mutex};
use termion::color;

pub struct BoundBuffer {
    size: usize,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    add: Arc<(Mutex<bool>, Condvar)>,
    remove: Arc<(Mutex<bool>, Condvar)>,
}

impl BoundBuffer {
    pub fn new(size: u8) -> BoundBuffer {
        BoundBuffer {
            size: size as usize,
            buffer: Arc::new(Mutex::new(VecDeque::<f32>::with_capacity(size as usize))),
            add: Arc::new((Mutex::new(true), Condvar::new())),
            remove: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    pub fn queue(&self, val: f32) -> () {
        // check buffer readiness (has space), explicitly drop mutex guard
        let (lock_add, cv_add) = &*self.add;
        let mut ready_add = lock_add.lock().unwrap();
        let buff = self.buffer.lock().unwrap();

        //let mut stdout = stdout();
        //write!(
        //    stdout,
        //    "{}Len @ queue: {}",
        //    termion::cursor::Goto(1, 31),
        //    buff.len()
        //)
        //.unwrap();
        //stdout.flush().unwrap();

        if buff.len() >= self.size {
            *ready_add = false;
        }
        std::mem::drop(buff);

        // thread wait until ready to add
        while !*ready_add {
            //(ready_add, _) = cv_add.wait_timeout(ready_add, time::Duration::from_millis(5) ).unwrap();
            ready_add = cv_add.wait(ready_add).unwrap();
            let buff = self.buffer.lock().unwrap();
            if buff.len() >= self.size {
                *ready_add = false;
            }
            std::mem::drop(buff);
        }
        std::mem::drop(ready_add);

        // push to buffer
        let mut buff = self.buffer.lock().unwrap();
        buff.push_back(val);
        std::mem::drop(buff);

        // update state and notify
        let (lock_remove, cv_remove) = &*self.remove;
        let mut ready_remove = lock_remove.lock().unwrap();
        *ready_remove = true;
        cv_remove.notify_one();
        std::mem::drop(ready_remove);
    }

    pub fn dequeue(&self) -> f32 {
        // check buffer readiness (has entries), explicitly drop mutex guard
        let (lock_remove, cv_remove) = &*self.remove;
        let mut ready_remove = lock_remove.lock().unwrap();
        let buff = self.buffer.lock().unwrap();
        if buff.is_empty() {
            *ready_remove = false;
        }
        //let mut stdout = stdout();
        //write!(
        //    stdout,
        //    "{}Len @ dequeue: {}",
        //    termion::cursor::Goto(1, 32),
        //    buff.len()
        //)
        //.unwrap();
        //stdout.flush().unwrap();
        std::mem::drop(buff);

        // thread wait until ready
        while !*ready_remove {
            //(ready_remove, _) = cv_remove.wait_timeout(ready_remove, time::Duration::from_millis(5)).unwrap();
            ready_remove = cv_remove.wait(ready_remove).unwrap();
            let buff = self.buffer.lock().unwrap();
            if buff.is_empty() {
                *ready_remove = false;
            }
            std::mem::drop(buff);
        }
        std::mem::drop(ready_remove);

        // pop from buffer
        let mut buff = self.buffer.lock().unwrap();
        let val = buff.pop_front().unwrap();
        std::mem::drop(buff);

        // update state and notify
        let (lock_add, cv_add) = &*self.add;
        let mut ready_add = lock_add.lock().unwrap();
        *ready_add = true;
        cv_add.notify_one();
        std::mem::drop(ready_add);

        return val;
    }
}

impl Clone for BoundBuffer {
    fn clone(&self) -> BoundBuffer {
        BoundBuffer {
            size: self.size,
            buffer: self.buffer.clone(),
            add: self.add.clone(),
            remove: self.remove.clone(),
        }
    }
}

pub struct Histogram {
    bins: usize,
    lower: f32,
    upper: f32,
    max: f32,
    counts: Vec<u32>,
    entries: usize,
    height: usize,
    width: usize,
    x_pad: usize,
}

/// Histogram/draw class for the process
///
/// monospaced terminals are 2:1
/// not tuning this right now, but draw space is 60 wide, 30 tall
/// use the background of a char too, this means we have 60x60
/// with a max_z = 1000, each half char is 16.677r counts
/// which means 15.35/256 increments
/// gets weirder though
impl Histogram {
    pub fn new(bins: u8, lower: f32, upper: f32, max: f32) -> Histogram {
        if lower > upper {
            panic!("Lower bound above upper bound");
        }
        if bins <= 0u8 {
            panic!("Bins cannot be negative or 0");
        }
        Histogram {
            bins: bins as usize,
            lower,
            upper,
            max,
            counts: vec![0u32; bins as usize],
            entries: 0usize,
            width: bins as usize,
            height: (bins / 2) as usize,
            x_pad: 10,
            //y_pad: 0
        }
    }

    pub fn draw_pad(&self) {
        let mut stdout = stdout();

        for x in 0..self.width {
            for y in 0..self.height {
                write!(
                    stdout,
                    "{}{}{}▄{}{}",
                    termion::cursor::Goto((x + self.x_pad) as u16, (self.height - y) as u16),
                    color::Fg(color::Rgb(0u8, 0u8, 0u8)),
                    color::Bg(color::Rgb(0u8, 0u8, 0u8)),
                    color::Fg(color::Reset),
                    color::Bg(color::Reset),
                )
                .unwrap();
            }
        }
        write!(
            stdout,
            "{}{}",
            termion::cursor::Goto(3, 1),
            f32::floor(self.max) as u16
        )
        .unwrap();
        write!(
            stdout,
            "{}{}",
            termion::cursor::Goto(3, f32::floor(self.height as f32 * 0.25) as u16),
            self.max * 0.75
        )
        .unwrap();
        write!(
            stdout,
            "{}{}",
            termion::cursor::Goto(3, f32::floor(self.height as f32 * 0.5) as u16),
            self.max * 0.5
        )
        .unwrap();
        write!(
            stdout,
            "{}{}",
            termion::cursor::Goto(3, f32::floor(self.height as f32 * 0.75) as u16),
            self.max * 0.25
        )
        .unwrap();
        write!(
            stdout,
            "{}{}",
            termion::cursor::Goto(3, f32::floor(self.height as f32) as u16),
            0
        )
        .unwrap();

        stdout.flush().unwrap();
    }

    // increments the bin that is to be filled, returns bin index
    pub fn fill(&mut self, val: f32) -> usize {
        // not bothering with underflow and overflow counts.
        if val < self.lower || val > self.upper {
            self.entries += 1;
            return 0usize;
        }

        let bin_width: f32 = (self.upper - self.lower) / (self.bins as f32);
        let bin = f32::floor((val - self.lower) / bin_width) as usize;

        self.counts[bin] += 1;
        self.entries += 1;

        bin + 1
    }

    /*
     * draw function for the histogram
     * a fun one, you only have to update a bin at a time
     * target its height and set its color and brightness
     * if you've done your job properly, the column below should be lit up already
     * use this character: ▄
     */
    pub fn draw(&self, bin: usize) -> () {
        // get the count
        if bin == 0 || bin > self.bins {
            return;
        }
        let count: u32 = self.counts[bin - 1];

        // convert count to pixel verticality
        let fraction = (count as f32) / self.max;
        let z: f32 = fraction * 30.0;
        let z_idx = f32::floor(z) as usize;

        let brightness = z - f32::floor(z);

        let fg: u8 = if brightness > 0.5 {
            255u8
        } else {
            ((brightness) * 255.0f32) as u8
        };
        let bg: u8 = if brightness > 0.5 {
            ((brightness) * 255.0f32) as u8
        } else {
            0u8
        };

        let mut stdout = stdout();
        write!(
            stdout,
            "{}{}{}▄{}{}{}",
            termion::cursor::Goto(self.x_pad as u16 + bin as u16 - 1, 30 - (z_idx as u16)),
            color::Fg(color::Rgb(fg, fg, fg)),
            color::Bg(color::Rgb(bg, bg, bg)),
            color::Fg(color::Reset),
            color::Bg(color::Reset),
            termion::cursor::Goto(1, 1)
        )
        .unwrap();

        write!(
            stdout,
            "{}Entries: {}",
            termion::cursor::Goto(1, 33),
            self.entries
        )
        .unwrap();
        stdout.flush().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct() {
        let bb: BoundBuffer = BoundBuffer::new(30);

        assert_eq!(bb.size, 30);

        let b = &bb.buffer.lock().unwrap();
        assert_eq!(b.len(), 0);
        assert_eq!(b.capacity(), 30);

        let addpair = &*bb.add;
        let addmutex = addpair.0.lock().unwrap();
        assert!(*addmutex);

        let removepair = &*bb.remove;
        let removemutex = removepair.0.lock().unwrap();
        assert!(!*removemutex);
    }

    #[test]
    fn test_add() {
        let mut bb: BoundBuffer = BoundBuffer::new(30);
        bb.queue(3f32);

        assert_eq!(bb.size, 30);

        let b = &bb.buffer.lock().unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b.capacity(), 30);

        let addpair = &*bb.add;
        let addmutex = addpair.0.lock().unwrap();
        assert!(*addmutex);

        let removepair = &*bb.remove;
        let removemutex = removepair.0.lock().unwrap();
        assert!(*removemutex);
    }

    #[test]
    fn test_remove() {
        let mut bb: BoundBuffer = BoundBuffer::new(30);
        bb.queue(3f32);
        let val = bb.dequeue();

        assert_eq!(bb.size, 30);
        assert_eq!(val, 3f32);

        let b = &bb.buffer.lock().unwrap();
        assert_eq!(b.len(), 0);
        assert_eq!(b.capacity(), 30);

        let addpair = &*bb.add;
        let addmutex = addpair.0.lock().unwrap();
        assert!(*addmutex);

        let removepair = &*bb.remove;
        let removemutex = removepair.0.lock().unwrap();
        assert!(!*removemutex);
    }
}
