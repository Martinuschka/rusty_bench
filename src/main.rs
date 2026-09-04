use std::io::{self, Write};
//use std::time::{Instant};
use rayon::prelude::*;
use std::cmp;

// ASCII Progress Bar
fn draw_progress_bar(percentage: f64, width: usize) {
    let bar_width = cmp::min(width as u8, 50);
    let filled = (percentage * bar_width as f64).floor() as u8;
    let empty = bar_width - filled;

    print!("\r[");
    for _ in 0..filled {
        print!("=");
    }
    for _ in 0..empty {
        print!(" ");
    }
    print!("] {:.2}%", percentage);
    io::stdout().flush().unwrap();
}

// Chudnovsky Algorithm
fn chudnovsky_algorithm(n_terms: usize) -> f64 {
    let mut sum = 0.0;
    for k in 0..n_terms {
        let numerator = (-1_i32.pow(k as u32) as f64) * ((6 * k - 1) as f64).powf(3.0);
        let denominator = (k as f64).powf(3.) * (2 * k + 1) as f64 * (2 * k + 3) as f64 * (2 * k + 5) as f64 * (2 * k + 7) as f64;
        sum += numerator / denominator;
    }
    let pi = 1.0 / (sum * 4.0 * f64::sqrt(10_000_000_000.0));
    pi
}

// Function to run the algorithm in parallel with rayon.
fn calculate_pi_parallel(n_terms: usize, num_threads: Option<usize>) -> f64 {
    let n_terms = if n_terms == 0 { 1_000_000 } else { n_terms };
    let mut sum = 0.0;

    // If num_threads is Some(x), use that number of threads.
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads.unwrap_or(4))
        .build()
        .unwrap()
        .scope(|s| {
            for chunk in n_terms / 8..n_terms + 1 {
                let start = (chunk - 1) * 8;
                let end = chunk * 8;

                s.spawn(move |_| {
                    let mut local_sum = 0.0;
                    for k in start..end {
                        let numerator = (-1_i32.pow(k as u32) as f64) * ((6 * k - 1) as f64).powf(3.0);
                        let denominator = (k as f64).powf(3.) * (2 * k + 1) as f64 * (2 * k + 3) as f64 * (2 * k + 5) as f64 * (2 * k + 7) as f64;
                        local_sum += numerator / denominator;
                    }
                    sum += local_sum;
                });
            }
        });

    let pi = 1.0 / (sum * 4.0 * f64::sqrt(10_000_000_000.0));
    pi
}

// User input handling function
fn get_user_input() -> (usize, Option<usize>) {
    let mut digits_input = String::new();
    let mut threads_input = String::new();

    loop {
        print!("Enter number of correct Pi digits to calculate (0 for continuous): ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut digits_input).expect("Failed to read line");

        match digits_input.trim().parse::<usize>() {
            Ok(_) => break,
            Err(_) => println!("Please enter a valid number."),
        }
    }

    loop {
        print!("Enter number of threads (0 for all cores): ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut threads_input).expect("Failed to read line");

        match threads_input.trim().parse::<usize>() {
            Ok(_) => break,
            Err(_) => println!("Please enter a valid number of threads."),
        }
    }

    let num_threads = if threads_input.trim() == "0" {
        None
    } else {
        Some(threads_input.trim().parse::<usize>().unwrap())
    };

    (digits_input.trim().parse::<usize>().unwrap(), num_threads)
}

// Main function with loop for restarting after interruption
fn main() {
    loop {
        let (num_digits, num_threads) = get_user_input();

        if num_digits == 0 && num_threads.is_none() {
            println!("Running benchmark until interrupted. Press Ctrl+C to stop.");
        } else {
            println!(
                "Starting calculation of Pi with {} digits using {} threads...",
                num_digits,
                match &num_threads {
                    Some(t) => t.to_string(),
                    None => "all available".to_string(),
                }
            );
        }

        //let start_time = Instant::now();
        let mut calculated_pi: f64 = 0.0;
        let mut total_terms = 0;

        // Progress tracking
        let progress_width = 50;

        loop {
            //let elapsed = start_time.elapsed().as_secs();

            if num_digits > 0 && total_terms >= num_digits * 1_000_000 / 4 { // rough estimate of terms needed per digit
                calculated_pi = calculate_pi_parallel(num_digits, num_threads);
                break;
            }

            let percentage = (total_terms as f64 / (num_digits * 1_000_000 / 4) as f64) * 100.0;

            draw_progress_bar(percentage, progress_width);

            // Simulate workload for the sake of progress bar
            total_terms += 1;
            //std::thread::sleep(Duration::from_millis(25)); // sleep to simulate work

            if num_digits == 0 {
                // Let user interrupt with Ctrl+C
                let mut input = String::new();
                match io::stdin().read_line(&mut input) {
                    Ok(_) => {
                        if input.trim() == "q" || input.trim() == "quit" {
                            break;
                        }
                    },
                    Err(e) => eprintln!("Error reading input: {}", e),
                }
            }
        }

        // Final result
        draw_progress_bar(100.0, progress_width);
        println!("\n\nEstimated Pi value: {:.20}\n", calculated_pi);

        // Ask user if they want to restart
        print!("Do you want to run again? (y/n): ");
        io::stdout().flush().unwrap();
        let mut choice = String::new();
        match io::stdin().read_line(&mut choice) {
            Ok(_) => {
                if choice.trim() == "n" || choice.trim() == "no" {
                    break;
                }
            },
            Err(e) => eprintln!("Error reading input: {}", e),
        }
    }
}
