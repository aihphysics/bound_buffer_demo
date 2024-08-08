use bbr::*;

use clap::{command, Parser};
use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::thread;
use std::thread::JoinHandle;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Buffer size
    #[arg(short = 'b', long = "buffsize", default_value_t = 100usize)]
    pub buffer_size: usize,

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

/// function for sampling unique gaussian and pushing to bounded-buffer
///
/// A unique gaussian distribution `gauss` is created, random parameters.
/// `samples` samples are taken from the distribution and inserted into a [`BoundBuffer`]
fn sampler(buffer: &BoundBuffer<f32>, samples: usize) {
    let mut rng = rand::thread_rng();
    let gauss = Normal::new(rng.gen_range(0f32..60f32), rng.gen_range(1f32..5f32)).unwrap();
    for _ in 0..samples {
        let val = gauss.sample(&mut rng);
        buffer.queue(val);
    }
}

/// function to visualise samples taken from a bounded-buffer
///
/// Uses a [`Histogram`] to plot samples pushed to a [`BoundBuffer`].
/// Fill function returns the bin the value belongs to
/// Draw function plots the change of this bin to terminal.
/// Plots `iterations` number of samples.
/// Artificial delay to simulate slow producer may be added with `delay` in ms.
fn plotter(buffer: &BoundBuffer<f32>, iterations: usize, delay: u64) {
    let mut hist: Histogram = Histogram::new(60, 0f32, 60f32, 1000f32);
    hist.draw_pad();
    for _ in 0..iterations {
        let val = buffer.dequeue();
        let bin = hist.fill(val);
        hist.draw(bin);
        thread::sleep(std::time::Duration::from_millis(delay));
    }
}

fn main() {
    // Prepare arguments
    let args = Args::parse();
    let buffer_size = args.buffer_size;
    let producers = args.producers;
    let delay = args.delay;
    let samples = args.samples;
    let iterations = producers * samples;

    print!("{}", termion::clear::All);

    // create buffer for sharing between threads
    let bound_buffer = Arc::new(BoundBuffer::<f32>::new(buffer_size));

    // collection for storing producer thread handles.
    let mut handles: Vec<JoinHandle<()>> = Vec::with_capacity(producers);

    // create producers, each thread samples a unique gaussian distribution, pushes to shared
    // buffer.
    for _ in 0..producers {
        let production_buffer = bound_buffer.clone();
        handles.push(thread::spawn(move || sampler(&production_buffer, samples)))
    }

    // create the consumer thread, visualises all generated samples.
    let vis_buff = bound_buffer.clone();
    let visualiser = thread::spawn(move || plotter(&vis_buff, iterations, delay));

    // wait for all threads to join.
    visualiser.join().unwrap();
    for handle in handles {
        handle.join().unwrap()
    }

    println!("\nIteration finished");
}
