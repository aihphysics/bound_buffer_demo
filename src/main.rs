use std::sync::{Arc, Condvar, Mutex};
use std::collections::vec_deque::VecDeque;
use std::time::Duration;
use std::thread;

use rand::Rng;

#[derive(Copy, Clone)]
struct Gaussian {
  a: f32,
  mu: f32,
  sigma: f32,
}

impl Gaussian {
  fn value( self, x: f32 ) -> f32 {
    return self.a * f32::powf( -0.5 * f32::powf((x - self.mu ) / self.sigma, 2.0), std::f32::consts::E )
  }
}


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

      // check buffer readiness (has space)
      let ( lock_add, cv_add )  = &*self.add;
      let mut ready = lock_add.lock().unwrap();
      let mut buff = self.buffer.lock().unwrap();
      if buff.len() == self.size { *ready = false; } 
    
      // thread wait until ready
      while !*ready {
        ready = cv_add.wait( ready ).unwrap();
      }

      // push to buffer 
      buff.push_back( val );

      // update state and notify
      let ( lock_remove, cv_remove )  = &*self.remove;
      *lock_remove.lock().unwrap() = true;
      cv_remove.notify_all();
      
    }

    fn dequeue( &self ) -> f32 {

      // check buffer readiness (has entries)
      let ( lock_remove, cv_remove )  = &*self.remove;
      let mut ready = lock_remove.lock().unwrap();
      let mut buff = self.buffer.lock().unwrap();
      if buff.is_empty() { *ready = false; }
    
      // thread wait until ready
      while !*ready {
        println!("wait dequeue");
        ready = cv_remove.wait( ready ).unwrap();
      }

      // pop from buffer 
      let val = buff.pop_front().unwrap();

      // update state and notify
      let ( lock_add, cv_add )  = &*self.add;
      *lock_add.lock().unwrap() = true;
      cv_add.notify_all();
      if buff.is_empty() { *ready = false; }

      return val;
    }

}


fn main() {

      
  let bb = Arc::new( BoundBuffer::new( 30 ) );
  let gauss_buff1= Arc::clone( &bb );
  let gauss_buff2= Arc::clone( &bb );
  let write_buff= Arc::clone( &bb );

  let gauss1 = thread::spawn( move || {
    let gauss = Gaussian{ a: 10f32, mu: 0f32, sigma: 10f32 };
    let mut rng = rand::thread_rng();
    for _ in 0..1000 {
      let val = gauss.value( rng.gen_range(0.0..30.0) );
      gauss_buff1.queue( val );
    }
  });

  let gauss2 = thread::spawn( move || {
    let mut rng = rand::thread_rng();
    let gauss = Gaussian{ a: 10f32, mu: 0f32, sigma: 10f32 };
    for _ in 0..1000 {
      let val = gauss.value( rng.gen_range(0.0..30.0) );
      gauss_buff2.queue( val );
    }
  });


  let writer = thread::spawn( move || {
    let val = write_buff.dequeue();
    //plot( val );
  });


  gauss1.join().unwrap();
  gauss2.join().unwrap();
  writer.join().unwrap();
  

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
