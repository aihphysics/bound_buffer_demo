#![doc = include_str!("../readme.md")]

use std::collections::vec_deque::VecDeque;
use std::io::Stdout;
use std::io::{stdout, Write};
use std::sync::{Arc, Condvar, Mutex};
use termion::color;

/// Bound-buffer struct
///
/// * `size` limits size of the circular queue
/// * `buffer` Fouble ended queue, mutex protected.
/// * `add` Condvar and mutex'd bool paired together for inter-thread signalling buffer is ready
/// for queuing
/// * `remove` Condvar and mutex'd bool paired together for inter-thread signalling buffer is ready
/// for dequeuing
pub struct BoundBuffer<T> {
    size: usize,
    buffer: Arc<Mutex<VecDeque<T>>>,
    add: Arc<(Mutex<bool>, Condvar)>,
    remove: Arc<(Mutex<bool>, Condvar)>,
}

impl<T> BoundBuffer<T> {
    /// constructor for generic bound buffer
    pub fn new(size: usize) -> BoundBuffer<T> {
        BoundBuffer::<T> {
            size: size as usize,
            buffer: Arc::new(Mutex::new(VecDeque::<T>::with_capacity(size as usize))),
            add: Arc::new((Mutex::new(true), Condvar::new())),
            remove: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    /// Function to perform thread-safe queue to bound-buffer
    ///
    /// Checks that buffer has space to push a value, halts thread if there is none.
    /// Rechecks on condition variable signal that buffer is ready for queue, then pushes values.
    /// Any waiting dequeuing threads are signalled after push.
    /// `std::mem::drop( _mutex_ )` is used to explicitly unlock all mutex in inter-thread
    /// readiness checking. Changes are signalled by the condvar.
    pub fn queue(&self, val: T) -> () {
        // check buffer readiness (has space), explicitly drop mutex guard
        let (lock_add, cv_add) = &*self.add;
        let mut ready_add = lock_add.lock().unwrap();
        let buff = self.buffer.lock().unwrap();
        if buff.len() >= self.size {
            *ready_add = false;
        }
        std::mem::drop(buff);

        // thread wait until ready to add
        while !*ready_add {
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

    /// Function to perform thread-safe dequeue from bound-buffer.
    ///
    /// Checks that buffer has values to be popped, halts thread if there is none.
    /// Rechecks on condition variable signal that buffer is ready for dequeue, then pops.
    /// Any waiting queuing threads are signalled after pop.
    /// `std::mem::drop( mutex )` is used to explicitly unlock all mutex in inter-thread
    /// readiness checking. Changes are signalled by the condvar.
    pub fn dequeue(&self) -> T {
        // check buffer readiness (has entries), explicitly drop mutex guard
        let (lock_remove, cv_remove) = &*self.remove;
        let mut ready_remove = lock_remove.lock().unwrap();
        let buff = self.buffer.lock().unwrap();
        if buff.is_empty() {
            *ready_remove = false;
        }
        std::mem::drop(buff);

        // thread wait until ready
        while !*ready_remove {
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
        let val: T = buff.pop_front().unwrap();
        std::mem::drop(buff);

        // update state and notify
        let (lock_add, cv_add) = &*self.add;
        let mut ready_add = lock_add.lock().unwrap();
        *ready_add = true;
        cv_add.notify_one();
        std::mem::drop(ready_add);

        let (lock_remove, _) = &*self.remove;
        let mut ready_remove = lock_remove.lock().unwrap();
        let buff = self.buffer.lock().unwrap();
        if buff.is_empty() {
            *ready_remove = false;
        }
        std::mem::drop(ready_remove);

        return val;
    }
}

impl<T> Clone for BoundBuffer<T> {
    fn clone(&self) -> BoundBuffer<T> {
        BoundBuffer::<T> {
            size: self.size,
            buffer: self.buffer.clone(),
            add: self.add.clone(),
            remove: self.remove.clone(),
        }
    }
}

/// Histogram class for visualising values on the terminal
///
/// Typical histogram definition. Configurable with upper and lower bounds, number of bins and so
/// on. `std::vec` used to store counts of binned samples. Various associated information for the
/// padding and the maximum value on the y-axis. No statistical tools included (yet).
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

impl Histogram {
    /// Histogram constructor
    ///
    /// Simple histogram constructor, number of `bins`, `lower` and `upper` bounds and the y-axis limit `max` are
    /// setable.
    /// # Panics
    /// Will panic if histogram cannot be constructed with provided parameters: negative/zero bins
    /// and invalid limits.
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

    /// Function to provide axis and background for the histogram
    ///
    /// Fills a region of the terminal with a black fg and bg character to provide a pad for
    /// drawing to. Draws an approximately accurate axis on the left hand side (rounding and so
    /// on).
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

    /// Function to bin a value into the histogram
    ///
    /// Histogram bins are indexed from 1. Underflow and overflow entries will be returned as 0.
    /// Takes a value `val` to be binned into the histogram, increments the count of the respective
    /// bin and returns its index.
    pub fn fill(&mut self, val: f32) -> usize {
        // catch any overflowing values
        if val < self.lower || val > self.upper {
            self.entries += 1;
            return 0usize;
        }

        // calculate binning
        let bin_width: f32 = (self.upper - self.lower) / (self.bins as f32);
        let bin = f32::floor((val - self.lower) / bin_width) as usize;

        // increment bin and entry count, return histogram bin.
        self.counts[bin] += 1;
        self.entries += 1;
        bin + 1
    }

    /// Function to light a pixel
    ///
    /// Details explained in draw()
    fn light(&self, mut stdout: &Stdout, bin: usize, z: usize, fg: u8, bg: u8) {
        write!(
            stdout,
            "{}{}{}▄",
            termion::cursor::Goto(
                (self.x_pad as u16 + bin as u16) - 1,
                30u16.saturating_sub(z as u16)
            ),
            color::Fg(color::Rgb(fg, fg, fg)),
            color::Bg(color::Rgb(bg, bg, bg)),
        )
        .unwrap();
    }

    /// Function to draw histogram to the terminal.
    ///
    /// This is an accumulative process, and this function should be used for live visualisation of
    /// binning as it is performed. The function takes the `bin` that has been incremented and
    /// draws the top of that particular bin to the screen. Hence, for a complete distribution
    /// the draw function should be called with the relevant bin each time the bin is filled.
    /// There is a single check of bin immediately below, so two characters are altered. This check
    /// to the bin below is to ensure that banding is removed.
    ///
    /// Characters on a terminal are 2:1, taller than they are wider. This function uses the half
    /// character '▄'. This allows the terminal to be effectively addressed as a canvas with 1:1
    /// pixels by setting the foreground and background values of this drawn character.
    ///
    /// The canvas is 30 lines tall. The count in a bin of the histogram is mapped from 0..max to
    /// 0..30.0, the fraction that the count is across a single bin is used to calculate the
    /// brightness of relevant the character. When the count is more than halfway across the bin, the
    /// foreground, corresponding to the lower half of the terminal character is set to white.
    /// The brightness is rescaled to each half-bin.
    ///
    /// It is possible to draw a single line with a single pass over each bin with the draw function.
    /// This will make a pretty weird looking line, and a specific function should be added for
    /// this.
    pub fn draw(&self, bin: usize) -> () {
        // get the count
        if bin == 0 || bin > self.bins {
            return;
        }
        let count: u32 = self.counts[bin - 1];

        // convert count to pixel verticality
        let fraction = ((count) as f32) / self.max;
        let z: f32 = fraction * 30.0;
        let z_idx = f32::floor(z) as usize;

        // Calculate the brightness of the character
        let brightness = z - f32::floor(z);

        // Map brightness to fg color.
        let fg: u8 = if brightness > 0.5 {
            255u8
        } else {
            ((brightness) * 2.0 * 255.0f32) as u8
        };

        // Map brightness to bg color.
        let bg: u8 = if brightness > 0.5 {
            ((brightness - 0.5) * 2.0 * 255.0f32) as u8
        } else {
            0u8
        };

        // Light the pixel incremented bin.
        let mut stdout = stdout();
        self.light(&stdout, bin, z_idx, fg, bg);
        if z_idx > 0 { // banding protection.
            self.light(&stdout, bin, z_idx-1, 255, 255);
        }

        // write the updated entry count to stdout
        write!(
            stdout,
            "{}{}{}Entries: {}",
            termion::cursor::Goto(1, 33),
            color::Fg(color::Reset),
            color::Bg(color::Reset),
            self.entries
        )
        .unwrap();

        // flush stdout buffer and finish
        stdout.flush().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct() {
        let bb: BoundBuffer<f32> = BoundBuffer::new(30);

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
        let bb: BoundBuffer<f32> = BoundBuffer::new(30);
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
        let bb: BoundBuffer<f32> = BoundBuffer::new(30);
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
        println!("{}", *removemutex);
        assert!(!*removemutex);
    }
}
