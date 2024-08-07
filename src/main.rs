use bbr::*;

use clap::{command, Parser};
use rand::Rng;
use std::thread;
use std::thread::JoinHandle;
use rand_distr::{Distribution, Normal};


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Buffer size
    #[arg(short = 'b', long = "buffsize", default_value_t = 100u8)]
    pub buffer_size: u8,

    /// Number of threads/producers
    #[arg(short = 'n', long = "producers", default_value_t = 6usize)]
    pub producers: usize,

    /// Producer delay in ms
    #[arg(short = 'd', long = "delay", default_value_t = 2u64)]
    pub delay: u64,

    /// samples per producer
    #[arg(short = 's', long = "samples", default_value_t = 2000)]
    pub samples: usize,
}

fn main() {
    let args = Args::parse();

    let buffer_size = args.buffer_size;
    let producers = args.producers;
    let delay = args.delay;
    let samples = args.samples;

    let iterations = producers * samples;

    // clear up before we begin
    print!("{}", termion::clear::All);

    let bound_buffer = BoundBuffer::new(buffer_size);

    let mut handles: Vec<JoinHandle<()>> = Vec::with_capacity(producers);

    for _ in 0..producers {
        let production_buffer = bound_buffer.clone();
        handles.push(thread::spawn(move || {
            let mut rng = rand::thread_rng();
            let gauss = Normal::new(rng.gen_range(0f32..60f32), rng.gen_range(1f32..5f32)).unwrap();
            for _ in 0..samples {
                let val = gauss.sample(&mut rng);
                production_buffer.queue(val);
            }
        }))
    }

    let write_buff = bound_buffer.clone();
    let writer = thread::spawn(move || {
        let mut hist: Histogram = Histogram::new(60, 0f32, 60f32, 1000f32);
        hist.draw_pad();
        for _ in 0..iterations {
            let val = write_buff.dequeue();
            let bin = hist.fill(val);
            hist.draw(bin);
            thread::sleep(std::time::Duration::from_millis(delay));
        }
    });

    writer.join().unwrap();
    for handle in handles {
        handle.join().unwrap()
    }

    println!("\nIteration finished");
}


