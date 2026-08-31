use core_affinity;
use ctrlc;
use rand::rngs::ThreadRng;
use rand::Rng;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn clear_screen() {
    // simple ANSI clear
    print!("\x1B[2J\x1B[H");
}

fn print_pi_logo() {
    // Medium pi shape using only '#' and spaces
    let logo = r#"
###############
    ### ###    
    ### ###    
    ### ###    
    ### ###    
    ### ###    
    ### ###    
"#;
    println!("{}", logo);
}

fn matched_digits(estimate: f64, reference: f64, max_check: usize) -> usize {
    // Compare decimal digits after the decimal point
    if !estimate.is_finite() {
        return 0;
    }
    // We will compare as strings up to max_check but limited by f64 precision (~15)
    let max_check = max_check.min(15);
    // Format with max_check digits
    let est_s = format!("{:.1$}", estimate, max_check);
    let ref_s = format!("{:.1$}", reference, max_check);
    // Remove the decimal point and the leading '3' so we count digits after the point equally
    let est_digits: String = est_s.chars().filter(|c| c.is_ascii_digit()).collect();
    let ref_digits: String = ref_s.chars().filter(|c| c.is_ascii_digit()).collect();
    // Compare starting from the first digit (which includes digits before decimal too),
    // but we'll count from start; that effectively checks initial digits including the '3'.
    let mut matched = 0usize;
    for (a, b) in est_digits.chars().zip(ref_digits.chars()) {
        if a == b {
            matched += 1;
        } else {
            break;
        }
    }
    matched
}

/// Prompt for a usize value, reading input until a valid number is entered.
fn prompt_usize(prompt: &str) -> usize {
    loop {
        print!("{}", prompt);
        io::stdout().flush().ok();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Failed to read input. Try again.");
            continue;
        }
        match input.trim().parse::<usize>() {
            Ok(n) => return n,
            Err(_) => {
                println!("Please enter a non-negative integer.");
            }
        }
    }
}

/// Prompt for the target digits or allow quitting by entering 'q' or 'Q'.
fn prompt_digits_or_quit(prompt: &str) -> Option<usize> {
    loop {
        print!("{}", prompt);
        io::stdout().flush().ok();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Failed to read input. Try again.");
            continue;
        }
        let s = input.trim();
        if s.eq_ignore_ascii_case("q") {
            return None;
        }
        match s.parse::<usize>() {
            Ok(n) => return Some(n),
            Err(_) => {
                println!("Please enter a non-negative integer or 'q' to quit.");
            }
        }
    }
}

fn human_bytes(n: u64) -> String {
    // format large integers in a readable way
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.2}K", n as f64 / 1_000f64)
    } else if n < 1_000_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000f64)
    } else {
        format!("{:.2}B", n as f64 / 1_000_000_000f64)
    }
}

fn main() {
    // Print an ASCII-stylized PI logo/banner right after startup
    print_pi_logo();
    
    println!("rusty_bench — Pi benchmark (Monte Carlo)");
    println!("Note: this benchmark uses a Monte Carlo estimator and verifies digits using f64 precision (~15 digits max).");
    println!("Press Ctrl+C at any time to interrupt the running benchmark and return to the prompts.\n");

        // Shared stop flag used by the Ctrl+C handler and by threads
    let stop_flag = Arc::new(AtomicBool::new(false));
    // install Ctrl+C handler once
    {
        let stop = stop_flag.clone();
        ctrlc::set_handler(move || {
            stop.store(true, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
    }

    loop {
        // reset stop
        stop_flag.store(false, Ordering::SeqCst);

        // allow quitting by entering 'q' at the digits prompt
        let target_digits = match prompt_digits_or_quit("How many correct digits of Pi should the benchmark run for? (0 = run until cancelled, 'q' to exit): ") {
            None => {
                println!("Exiting rusty_bench. Goodbye.");
                return;
            }
            Some(n) => n,
        };

        let num_threads = prompt_usize("How many threads should be used? (1.. = number of worker threads): ");
        if num_threads == 0 {
            println!("Number of threads must be >= 1.");
            continue;
        }

        // prepare counters
        let hits = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(0));

        // get available cores for affinity
        let cores = core_affinity::get_core_ids().unwrap_or_default();
        let core_count = cores.len();

        let start = Instant::now();

        // Spawn worker threads
        let mut handles = Vec::with_capacity(num_threads);
        for i in 0..num_threads {
            let hits = hits.clone();
            let total = total.clone();
            let stop = stop_flag.clone();
            let core_opt = cores.get(i % core_count).cloned();
            let handle = thread::spawn(move || {
                // try to set affinity for this worker
                if let Some(core) = core_opt {
                    // best-effort: ignore failures
                    core_affinity::set_for_current(core);
                }
                let mut rng: ThreadRng = rand::thread_rng();
                while !stop.load(Ordering::Relaxed) {
                    // generate a batch of samples to amortize overhead
                    let batch = 1_000usize;
                    let mut local_hits = 0u64;
                    for _ in 0..batch {
                        let x: f64 = rng.gen();
                        let y: f64 = rng.gen();
                        if x * x + y * y <= 1.0 {
                            local_hits += 1;
                        }
                    }
                    hits.fetch_add(local_hits, Ordering::Relaxed);
                    total.fetch_add(batch as u64, Ordering::Relaxed);
                }
            });
            handles.push(handle);
        }

        // Monitoring loop: update ASCII progress
        let mut last_total = 0u64;
        let spinner = vec!["|", "/", "-", "\\"];
        let mut spin_idx = 0usize;
        let reference_pi = std::f64::consts::PI;

        loop {
            if stop_flag.load(Ordering::SeqCst) {
                break;
            }
            let t = total.load(Ordering::Relaxed);
            let h = hits.load(Ordering::Relaxed);
            let elapsed = start.elapsed().as_secs_f64();
            let estimate = if t == 0 { 0.0 } else { 4.0 * (h as f64) / (t as f64) };
            let matched = matched_digits(estimate, reference_pi, if target_digits==0 {15} else {target_digits});

            // Progress text
            clear_screen();
            println!("rusty_bench — Pi benchmark (Monte Carlo)");
            println!("Threads: {}  Cores detected: {}", num_threads, core_count);
            println!("Target digits: {}  (0 = run until cancelled)", target_digits);
            println!("Elapsed: {:.2}s  Samples: {}  Hits: {}", elapsed, human_bytes(t), human_bytes(h));
            println!("Estimate: {:.12}  Matched initial digits: {}", estimate, matched);
            // Show rate
            let rate = if elapsed > 0.0 { (t as f64) / elapsed } else { 0.0 };
            println!("Rate: {:.2} samples/sec", rate);

            // ASCII progress bar when a target is set and >0
            if target_digits > 0 {
                let pct = (matched as f64) / (target_digits as f64);
                let pct = pct.clamp(0.0, 1.0);
                let width = 40usize;
                let filled = (pct * (width as f64)).round() as usize;
                let bar = format!("[{}{}] {:.1}%",
                                  "#".repeat(filled),
                                  " ".repeat(width.saturating_sub(filled)),
                                  pct * 100.0);
                println!("Target progress: {}", bar);
            } else {
                // spinner to show liveness
                println!("Running... {}", spinner[spin_idx % spinner.len()]);
                spin_idx = spin_idx.wrapping_add(1);
            }

            println!("(Press Ctrl+C to stop the benchmark and return to prompts)");

            // Check stop condition when target given: matched digits >= target_digits
            if target_digits > 0 && matched >= target_digits {
                // Signal stop
                stop_flag.store(true, Ordering::SeqCst);
            }

            // Sleep a bit
            thread::sleep(Duration::from_millis(250));

            // quick optimization: detect no progress for a while
            if t == last_total {
                // still update UI
            } else {
                last_total = t;
            }
        }

        // Wait for workers to finish
        for handle in handles {
            let _ = handle.join();
        }

        // final stats
        let t = total.load(Ordering::Relaxed);
        let h = hits.load(Ordering::Relaxed);
        let elapsed = start.elapsed().as_secs_f64();
        let estimate = if t == 0 { 0.0 } else { 4.0 * (h as f64) / (t as f64) };

        clear_screen();
        println!("rusty_bench — Completed run");
        println!("Elapsed: {:.2}s", elapsed);
        println!("Threads: {}  Samples: {}  Hits: {}", num_threads, human_bytes(t), human_bytes(h));
        println!("Final estimate: {:.12}", estimate);
        println!("Reference PI: {:.15}", std::f64::consts::PI);
        println!("(You can start a new run now.)\n");

        // small pause to ensure the user sees the final output before re-prompting
        // reset stop_flag for next round is done at loop top
    }
}
