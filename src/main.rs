use std::sync::{Arc};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::thread;
use std::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};

fn main() {
    loop {
        // Prompt for user input
        let mut digits_input = String::new();
        print!("Enter number of correct digits of Pi (0 to exit): ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut digits_input).expect("Failed to read line");

        let mut thread_input = String::new();
        print!("Enter number of threads: ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut thread_input).expect("Failed to read line");

        // Parse inputs
        let target_digits: u32 = match digits_input.trim().parse() {
            Ok(n) => n,
            Err(_) => 0,
        };

        let num_threads: usize = match thread_input.trim().parse() {
            Ok(n) => n,
            Err(_) => 1, // Default to one thread if invalid
        };

        if target_digits == 0 {
            run_indefinite(num_threads);
        } else {
            run_for_digits(target_digits, num_threads);
        }
    }
}

fn run_indefinite(thread_count: usize) {
    let exit_flag = Arc::new(AtomicBool::new(false));
    let counter = Arc::new(AtomicUsize::new(0));

    // Create a progress bar that updates indefinitely
    let pb = ProgressBar::new(1_000_000_000);
    pb.set_style(
        ProgressStyle::default().template("{spinner} {msg} | {wide_bar}")
            .unwrap()
    );

    // Signal handler thread to detect Ctrl+C
    let exit_flag_clone = exit_flag.clone();
    thread::spawn(move || {
        if let Ok(sig) = signal_hook::iterator::Signals::new(&[signal_hook::consts::SIGINT])
            .expect("Failed to create signal handler")
            .wait()
        {
            if sig == signal_hook::consts::SIGINT {
                exit_flag_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    });

    // Spawn threads for computation
    let pb_clone = pb.clone();
    let counter_clone = counter.clone();

    for i in 0..thread_count {
        let counter = counter_clone.clone();
        let exit_flag = exit_flag.clone();

        thread::spawn(move || {
            let mut sum: f64 = 0.0;
            let thread_id = i + 1;

            while !exit_flag.load(std::sync::atomic::Ordering::Relaxed) {
                // Simulate Leibniz series computation
                for k in (thread_id * 1_000_000)..((thread_id + 1) * 1_000_000) {
                    sum += 4.0 * (-1.0).powi(k as i32) / (2 * k as f64 + 1.0);

                    // Update progress every million terms
                    if k % 1_000_000 == 0 {
                        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }

                // Sleep to prevent CPU burn
                thread::sleep(Duration::from_millis(5));
            }
        });
    }

    // Update progress bar in main thread
    while !exit_flag.load(std::sync::atomic::Ordering::Relaxed) {
        pb.inc(counter.load(std::sync::atomic::Ordering::Relaxed) as u64 - pb.position());
        thread::sleep(Duration::from_millis(10));
    }

    // Print final result (though not accurate for indefinite runs)
    println!("\nComputation stopped by user.");
}

fn run_for_digits(digits: u32, thread_count: usize) {
    let mut total_terms_per_thread = 0;

    // Estimate terms needed based on digits (very rough approximation)
    if digits <= 10 {
        total_terms_per_thread = 1_000_000; // For low digits
    } else {
        total_terms_per_thread = 10_000_000; // For higher precision
    }

    let total_terms: usize = total_terms_per_thread * thread_count;
    let exit_flag = Arc::new(AtomicBool::new(false));
    let counter = Arc::new(AtomicUsize::new(0));

    // Create progress bar with known length
    let pb = ProgressBar::new(total_terms as u64);
    let pb = ProgressBar::new(total_terms as u64);
    pb.set_style(
        ProgressStyle::default().template("{spinner} {msg} | {wide_bar} {percent}%")
            .unwrap()
    );

    // Signal handler thread to detect Ctrl+C
    let exit_flag_clone = exit_flag.clone();
    thread::spawn(move || {
        if let Ok(sig) = signal_hook::iterator::Signals::new(&[signal_hook::consts::SIGINT])
            .expect("Failed to create signal handler")
            .wait()
        {
            if sig == signal_hook::consts::SIGINT {
                exit_flag_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    });

    // Collect partial sums from threads
    let (tx, rx) = crossbeam::channel::unbounded();

    for i in 0..thread_count {
        let tx = tx.clone();
        let counter = counter.clone();
        let exit_flag = exit_flag.clone();

        thread::spawn(move || {
            let mut sum: f64 = 0.0;

            // Each thread handles a portion of terms
            for k in (i as u64 * total_terms_per_thread) .. ((i + 1) as u64 * total_terms_per_thread) {
                if exit_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                sum += 4.0 * (-1.0).powi(k as i32) / (2 * k as f64 + 1.0);

                // Update progress every million terms
                if k % 1_000_000 == 0 {
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }

            tx.send(sum).unwrap();
        });
    }

    // Update progress bar while waiting for results
    let mut total_sum = 0.0;

    while !exit_flag.load(std::sync::atomic::Ordering::Relaxed) {
        pb.inc(counter.load(std::sync::atomic::Ordering::Relaxed) as u64 - pb.position());
        thread::sleep(Duration::from_millis(10));

        // Check if all threads have finished
        let mut received = 0;
        for _ in 0..thread_count {
            match rx.try_recv() {
                Ok(sum) => total_sum += sum,
                Err(crossbeam::channel::TryRecvError::Empty) => continue,
                Err(_) => break,
            }
            received += 1;
        }

        if received == thread_count {
            break; // All threads finished
        }
    }

    pb.finish_with_message("Benchmark complete!");

    // Print final Pi estimate (this is just for demonstration)
    println!("Estimated Pi: {:.10}", total_sum);
}