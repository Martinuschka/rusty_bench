use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::io::{self, Write};
use ctrlc;
use num_cpus;

fn main() {
    loop {
        let (digits, threads) = get_user_input();
        if digits == 0 {
            run_indefinite(threads);
        } else {
            let m = calculate_terms_needed(digits);
            run_parallel(m, threads);
        }
    }
}

fn get_user_input() -> (u32, usize) {
    loop {
        print!("Enter number of correct digits of Pi to compute (0 for infinite): ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("Error reading input");
            continue;
        }
        let input = input.trim();

        match input.parse::<u32>() {
            Ok(digits) => {
                // Now get threads
                loop {
                    print!("Enter number of threads to use (1-{}): ", num_cpus::get());
                    io::stdout().flush().unwrap();
                    let mut thread_input = String::new();
                    if io::stdin().read_line(&mut thread_input).is_err() {
                        eprintln!("Error reading input");
                        continue;
                    }
                    let thread_input = thread_input.trim();

                    match thread_input.parse::<usize>() {
                        Ok(threads) if threads > 0 => return (digits, threads),
                        _ => println!("Invalid number of threads. Please enter a positive integer."),
                    }
                }
            },
            Err(e) => eprintln!("Please enter a valid number for digits: {}", e),
        }
    }
}

fn calculate_terms_needed(digits: u32) -> u64 {
    if digits == 0 {
        return 1; // Avoid division by zero
    }
    let epsilon = 5.0 * 10.0_f64.powf(- (digits as f64));
    let m = ((1.0 / epsilon - 1.0) / 2.0).ceil() as u64;
    m
}

fn run_parallel(m: u64, threads: usize) {
    let progress_bar_length = 50;
    let total_terms = m;

    // Shared variables for tracking sum and progress
    let total_sum = Arc::new(Mutex::new(0.0f64));
    let (tx, rx) = std::sync::mpsc::channel();
    let counter = Arc::new(Mutex::new(0));

    let mut handles = vec![];
    for i in 0..threads {
        let start = i as u64 * m / threads as u64;
        let end = if i == threads - 1 {
            m
        } else {
            (i + 1) as u64 * m / threads as u64
        };
        let tx_clone = tx.clone();
        let counter_clone = Arc::clone(&counter);
        let total_sum_clone = Arc::clone(&total_sum);

        handles.push(thread::spawn(move || {
            let mut sum = 0.0f64;
            for k in start..end {
                let term = if k % 2 == 0 {
                    1.0 / (2.0 * k as f64 + 1.0)
                } else {
                    -1.0 / (2.0 * k as f64 + 1.0)
                };
                sum += term;

                // Send progress updates periodically
                if k % 100 == 0 {
                    tx_clone.send(k).unwrap();
                }
            }

            // Update global counter
            let mut c = counter_clone.lock().unwrap();
            *c += end - start;
            tx_clone.send(end).unwrap();

            // Add this thread's sum to the total
            let mut ts = total_sum_clone.lock().unwrap();
            *ts += sum;
        }));
    }

    drop(tx); // No longer need sender

    // Display progress while waiting for threads to finish
    let mut current_processed = 0;
    let reference_pi = std::f64::consts::PI;

    loop {
        match rx.try_recv() {
            Ok(progress) => {
                if progress > current_processed {
                    current_processed = progress;
                    let percentage = (current_processed as f64 / total_terms as f64) * 100.0;
                    print_progress(percentage, progress_bar_length);
                    // Calculate how many correct digits we have so far
                    let ts = total_sum.lock().unwrap();
                    let pi_approx = 4.0 * *ts;
                    let correct_digits = count_correct_digits(pi_approx, reference_pi);
                    println!("Current approximation: {:.15} ({} correct digits)", pi_approx, correct_digits);
                }
            },
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            },
            _ => break,
        }

        if current_processed >= total_terms {
            break;
        }
    }

    // Wait for all threads to finish
    for handle in handles {
        handle.join().unwrap();
    }

    // Final approximation and correct digits
    let ts = total_sum.lock().unwrap();
    let pi_approx = 4.0 * *ts;
    let correct_digits = count_correct_digits(pi_approx, reference_pi);
    println!("\nCalculated Pi approximation: {:.15} ({} correct digits)", pi_approx, correct_digits);
}

fn run_indefinite(threads: usize) {
    // Setup signal handler
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancelled_clone = Arc::clone(&cancelled);

    ctrlc::set_handler(move || {
        cancelled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    }).expect("Error setting Ctrl+C handler");

    // Shared variables
    let total_sum = Arc::new(Mutex::new(0.0f64));
    let (tx, rx) = std::sync::mpsc::channel();
    let progress_bar_length = 50;
    let reference_pi = std::f64::consts::PI;

    let mut handles = vec![];
    for i in 0..threads {
        let tx_clone = tx.clone();
        let total_sum_clone = Arc::clone(&total_sum);
        let cancelled_thread = Arc::clone(&cancelled);

        handles.push(thread::spawn(move || {
            let mut sum = 0.0f64;
            let mut terms_processed = 0;
            loop {
                if cancelled_thread.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                // Compute term k for this thread
                let k = i + terms_processed * threads;
                let term = if k % 2 == 0 {
                    1.0 / (2.0 * k as f64 + 1.0)
                } else {
                    -1.0 / (2.0 * k as f64 + 1.0)
                };
                sum += term;

                // Send progress update every so often
                if terms_processed % 100 == 0 {
                    tx_clone.send(k).unwrap();
                }

                terms_processed += 1;
            }
            let mut ts = total_sum_clone.lock().unwrap();
            *ts += sum;
        }));
    }

    drop(tx); // No longer need sender

    // Display progress while threads are running
    let mut current_processed = 0;

    loop {
        match rx.try_recv() {
            Ok(global_k) => {
                if global_k > current_processed {
                    current_processed = global_k;
                    print_progress((current_processed as f64 / 1_000_000.0) * 100.0, progress_bar_length);
                    let ts = total_sum.lock().unwrap();
                    let pi_approx = 4.0 * *ts;
                    let correct_digits = count_correct_digits(pi_approx, reference_pi);
                    println!("Current approximation: {:.15} ({} correct digits)", pi_approx, correct_digits);
                }
            },
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            },
            _ => break,
        }

        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
    }

    // Wait for all threads to finish
    for handle in handles {
        handle.join().unwrap();
    }

    println!("\nBenchmark stopped.");
}

fn print_progress(percentage: f64, bar_length: usize) {
    let filled = (percentage / 100.0 * bar_length as f64).floor() as usize;
    let mut progress_str = String::new();
    progress_str.push('[');
    for _ in 0..filled {
        progress_str.push('=');
    }
    for _ in filled..bar_length {
        progress_str.push(' ');
    }
    progress_str.push(']');
    progress_str.push_str(&format!(" {:.2}%", percentage));
    print!("\r{}", progress_str);
    io::stdout().flush().unwrap();
}

fn count_correct_digits(approx: f64, true_value: f64) -> u32 {
    let mut i = 0u32; // Explicitly declare as u32
    while i < 15 { // Check up to 15 digits
        let approx_digit = (approx * 10.0_f64.powi(i as i32)) as u64 % 10;
        let true_digit = (true_value * 10.0_f64.powi(i as i32)) as u64 % 10;
        if approx_digit != true_digit {
            break;
        }
        i += 1;
    }
    i
}
