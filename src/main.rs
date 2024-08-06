use std::sync::{Arc, Condvar, Mutex};
use std::collections::vec_deque::VecDeque;
use std::{thread, time};
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

      // check buffer readiness (has space)
      let ( lock_add, cv_add )  = &*self.add;
      let mut ready_add = lock_add.lock().unwrap();
      let buff = self.buffer.lock().unwrap();
      if buff.len() == self.size { *ready_add = false; } 
      std::mem::drop(buff);
    
      // thread wait until ready to add
      while !*ready_add {
        //println!("wait queue");
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

      // check buffer readiness (has entries), explicitly drop
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


fn main() {

      
  let bb = Arc::new( BoundBuffer::new( 5 ) );
  let gauss_buff1= Arc::clone( &bb );
  let gauss_buff2= Arc::clone( &bb );
  let write_buff= Arc::clone( &bb );

  
  let gauss1 = thread::spawn( move || {
    let mut rng = rand::thread_rng();
    let gauss = Normal::new( 2.0, 3.0 ).unwrap();
    for _ in 0..1000 {
      let val = gauss.sample( &mut rng );
      //println!( "g1: {val}" );
      thread::sleep(time::Duration::from_millis(25) );
      gauss_buff1.queue( val );
    }
  });

  let gauss2 = thread::spawn( move || {
    let mut rng = rand::thread_rng();
    let gauss = Normal::new( 6.0, 6.0 ).unwrap();
    for _ in 0..1000 {
      let val = gauss.sample( &mut rng );
      //println!( "g2: {val}" );
      thread::sleep(time::Duration::from_millis(25) );
      gauss_buff2.queue( val );
    }
  });

  let writer = thread::spawn( move || {
    for _ in 0..1000 {
      let val = write_buff.dequeue();
      //println!( "dq: {val}" );
      thread::sleep(time::Duration::from_millis(100) );
    }
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
