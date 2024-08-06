
use std::sync::{Arc, Condvar, Mutex};
use std::collections::vec_deque::VecDeque;
use std::{thread, time};
use std::io::{stdout, Write};

use termion::{color, cursor}; 
use termion::raw::IntoRawMode;
use rand_distr::{ Distribution, Normal };


struct BoundBuffer {
    size: usize,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    add: Arc<(Mutex<bool>, Condvar)>,
    remove: Arc<(Mutex<bool>, Condvar)>,
}

impl BoundBuffer {
    fn new(size: u8) -> BoundBuffer {
        BoundBuffer {
          size: size as usize,
          buffer: Arc::new(Mutex::new(VecDeque::<f32>::with_capacity(size as usize))),
          add: Arc::new((Mutex::new(true), Condvar::new())),
          remove: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    fn queue( &self, val: f32 ) ->(){

      // check buffer readiness (has space), explicitly drop mutex guard
      let ( lock_add, cv_add )  = &*self.add;
      let mut ready_add = lock_add.lock().unwrap();
      let buff = self.buffer.lock().unwrap();
      if buff.len() == self.size { *ready_add = false; } 
      std::mem::drop(buff);
    
      // thread wait until ready to add
      while !*ready_add {
        ready_add = cv_add.wait( ready_add ).unwrap();
      }

      // push to buffer 
      let mut buff = self.buffer.lock().unwrap();
      buff.push_back( val );

      // update state and notify
      let ( lock_remove, cv_remove )  = &*self.remove;
      let mut ready_remove = lock_remove.lock().unwrap();
      *ready_remove = true;
      cv_remove.notify_one();
      
    }

    fn dequeue( &self ) -> f32 {

      // check buffer readiness (has entries), explicitly drop mutex guard
      let ( lock_remove, cv_remove )  = &*self.remove;
      let mut ready_remove = lock_remove.lock().unwrap();
      let buff = self.buffer.lock().unwrap();
      if buff.is_empty() { *ready_remove = false; }
      cv_remove.notify_all();
      std::mem::drop(buff);
    
      // thread wait until ready
      while !*ready_remove {
        ready_remove = cv_remove.wait( ready_remove ).unwrap();
      }

      // pop from buffer 
      let mut buff = self.buffer.lock().unwrap();
      let val = buff.pop_front().unwrap();

      // update state and notify
      let ( lock_add, cv_add )  = &*self.add;
      *lock_add.lock().unwrap() = true;
      cv_add.notify_one();

      return val;
    }

}


struct Histogram {
  bins: usize,
  lower: f32,
  upper: f32,
  max: f32,
  counts: Vec<u32>,
  entries: usize
  
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

  fn new( bins: u8, lower: f32, upper:f32, max:f32 ) -> Histogram {
    if lower > upper { panic!("Lower bound above upper bound"); }
    if bins <= 0u8 { panic!("Bins cannot be negative or 0"); }
    Histogram{ bins: bins as usize, lower, upper, max, counts: vec![0u32; bins as usize], entries: 0usize }
  }

  // increments the bin that is to be filled, returns bin index
  fn fill( &mut self, val: f32 ) -> usize{
    
    // not bothering with underflow and overflow counts.
    if val < self.lower || val > self.upper { return 0usize; }

    let bin_width: f32 = (self.upper - self.lower)/(self.bins as f32);
    let bin = f32::floor( (val-self.lower)/bin_width) as usize;

    self.counts[bin] += 1;
    self.entries+=1;

    bin+1
  }

  /*
   * draw function for the histogram
   * a fun one, you only have to update a bin at a time
   * target its height and set its color and brightness
   * if you've done your job properly, the column below should be lit up already
   * use this character: ▄
   */
  fn draw( &self, bin: usize ) -> () {
    
    // get the count
    if bin == 0 || bin > self.bins { return; }
    let count: u32 = self.counts[bin-1];

    // convert count to pixel verticality
    let fraction = (count as f32)/self.max;
    let z: f32 = fraction * 30.0;
    let z_idx = f32::floor( z ) as usize;

    let brightness = z - f32::floor( z );

    let fg: u8 = if brightness > 0.5 { ((brightness-0.5f32)*255.0f32) as u8} else { 0u8 };
    let bg: u8 = if brightness > 0.5 { ((brightness-0.5f32)*255.0f32) as u8 } else { 0u8 };

    let stdout = stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();
    
    write!( stdout, "{}{}{}▄{}{}{}", 
      termion::cursor::Goto(bin as u16 + 1, z_idx as u16 + 1), 
      color::Fg(color::Rgb( fg, fg, fg ) ), 
      color::Bg(color::Rgb( bg, bg, bg ) ), 
      color::Fg(color::Reset), 
      color::Bg(color::Reset), 
      termion::cursor::Goto(1, 1)  ).unwrap();
    stdout.flush().unwrap();

    write!( stdout, "{}Entries: {}", termion::cursor::Goto(1, 32), self.entries  ).unwrap();
    stdout.flush().unwrap();

  }

}


fn main() {

  // clear up before we begin
  print!("{}", termion::clear::All );
      
  let bb = Arc::new( BoundBuffer::new( 5 ) );
  let gauss_buff1= Arc::clone( &bb );
  let gauss_buff2= Arc::clone( &bb );
  let write_buff= Arc::clone( &bb );

  
  let gauss1 = thread::spawn( move || {
    let mut rng = rand::thread_rng();
    let gauss = Normal::new( 5.0, 3.0 ).unwrap();
    for _ in 0..2000 {
      let val = gauss.sample( &mut rng );
      //println!( "g1: {val}" );
      //thread::sleep(time::Duration::from_millis(4) );
      gauss_buff1.queue( val );
    }
  });

  let gauss2 = thread::spawn( move || {
    let mut rng = rand::thread_rng();
    let gauss = Normal::new( 20.0, 6.0 ).unwrap();
    for _ in 0..2000 {
      let val = gauss.sample( &mut rng );
      //println!( "g2: {val}" );
      //thread::sleep(time::Duration::from_millis(4) );
      gauss_buff2.queue( val );
    }
  });

  let writer = thread::spawn( move || {

    let mut hist: Histogram = Histogram::new( 60, 0f32, 60f32, 1000f32 );
    for _ in 0..4000 {
      let val = write_buff.dequeue();
      //println!( "dq: {val}" );
      let bin = hist.fill( val );
      hist.draw( bin );
      //thread::sleep(time::Duration::from_millis(10) );
    }
  });

  writer.join().unwrap();
  gauss2.join().unwrap();
  gauss1.join().unwrap();


  

}






#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct() {
      let bb: BoundBuffer = BoundBuffer::new( 30 );

      assert_eq!( bb.size, 30 );

      let b =  &bb.buffer.lock().unwrap();
      assert_eq!( b.len(), 0 );
      assert_eq!( b.capacity(), 30 );

      let addpair  = &*bb.add;
      let addmutex = addpair.0.lock().unwrap();
      assert!( *addmutex );
  
      let removepair  = &*bb.remove;
      let removemutex = removepair.0.lock().unwrap();
      assert!( !*removemutex );
    } 

    #[test]
    fn test_add() {
      
      let mut bb: BoundBuffer = BoundBuffer::new( 30 );
      bb.queue( 3f32 );

      assert_eq!( bb.size, 30 );

      let b =  &bb.buffer.lock().unwrap();
      assert_eq!( b.len(), 1 );
      assert_eq!( b.capacity(), 30 );

      let addpair  = &*bb.add;
      let addmutex = addpair.0.lock().unwrap();
      assert!( *addmutex );
  
      let removepair  = &*bb.remove;
      let removemutex = removepair.0.lock().unwrap();
      assert!( *removemutex );

    }
    
    #[test]
    fn test_remove() {

      let mut bb: BoundBuffer = BoundBuffer::new( 30 );
      bb.queue( 3f32 );
      let val = bb.dequeue();

      assert_eq!( bb.size, 30 );
      assert_eq!( val, 3f32 );

      let b =  &bb.buffer.lock().unwrap();
      assert_eq!( b.len(), 0 );
      assert_eq!( b.capacity(), 30 );

      let addpair  = &*bb.add;
      let addmutex = addpair.0.lock().unwrap();
      assert!( *addmutex );
  
      let removepair  = &*bb.remove;
      let removemutex = removepair.0.lock().unwrap();
      assert!( !*removemutex );


    }


}
