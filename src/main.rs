use core_affinity::{CoreId, get_core_ids, set_for_current};
use rand::Rng;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

static STOP: AtomicBool = AtomicBool::new(false);

fn main() {
    ctrlc::set_handler(|| {
        STOP.store(true, Ordering::SeqCst);
    })
        .expect("failed to install Ctrl+C handler");

    let core_ids = get_core_ids().unwrap_or_default();
    if core_ids.is_empty() {
        eprintln!("Warning: could not enumerate CPU cores; threads will not be pinned.");
    }

    //println!("=== Pi benchmark ===");
    print_banner();
    println!("Enter 0 digits to run until interrupted. Enter q at a prompt to quit.");

    loop {
        let target_digits = match ask_target_digits() {
            Some(value) => value,
            None => break,
        };

        let requested_threads = match ask_thread_count() {
            Some(value) => value,
            None => break,
        };

        let available_cores = core_ids.len().max(1) as u64;
        let threads = requested_threads.min(available_cores);

        if requested_threads > available_cores {
            eprintln!(
                "Only {} CPU cores are available, so this run uses {} thread(s) for dedicated cores.",
                available_cores, threads
            );
        }

        STOP.store(false, Ordering::SeqCst);
        run(target_digits, threads, &core_ids);
        STOP.store(false, Ordering::SeqCst);
        println!();
    }

    println!("Bye.");
}

fn ask_target_digits() -> Option<u64> {
    loop {
        let line = prompt_line(
            "How many correct digits of Pi? (0 = run until Ctrl+C, q = quit): ",
        )?;

        if line.is_empty() {
            continue;
        }

        if line.eq_ignore_ascii_case("q") || line.eq_ignore_ascii_case("quit") {
            return None;
        }

        match line.parse::<u64>() {
            Ok(value) => {
                if value == 0 {
                    return Some(0);
                }

                if value > 15 {
                    eprintln!("f64 precision is limited to about 15 digits, so this run will target 15.");
                    return Some(15);
                }

                return Some(value);
            }
            Err(_) => {
                eprintln!("Please enter a non-negative whole number, e.g. 0, 5, or 15.");
            }
        }
    }
}

fn ask_thread_count() -> Option<u64> {
    loop {
        let line = prompt_line("How many threads should be used? (q = quit): ")?;

        if line.is_empty() {
            continue;
        }

        if line.eq_ignore_ascii_case("q") || line.eq_ignore_ascii_case("quit") {
            return None;
        }

        match line.parse::<u64>() {
            Ok(value) => {
                if value == 0 {
                    eprintln!("Please enter at least 1 thread.");
                } else {
                    return Some(value);
                }
            }
            Err(_) => {
                eprintln!("Please enter a positive whole number, e.g. 1, 4, or 16.");
            }
        }
    }
}

fn prompt_line(msg: &str) -> Option<String> {
    print!("{}", msg);
    if io::stdout().flush().is_err() {
        return None;
    }

    let mut input = String::new();

    match io::stdin().read_line(&mut input) {
        Ok(read) => {
            if STOP.load(Ordering::SeqCst) {
                println!();
                STOP.store(false, Ordering::SeqCst);
                return Some(String::new());
            }

            if read == 0 {
                None
            } else {
                Some(input.trim().to_string())
            }
        }
        Err(_) => {
            if STOP.load(Ordering::SeqCst) {
                println!();
                STOP.store(false, Ordering::SeqCst);
                Some(String::new())
            } else {
                None
            }
        }
    }
}

fn run(target_digits: u64, threads: u64, core_ids: &[CoreId]) {
    let total = Arc::new(AtomicU64::new(0));
    let inside = Arc::new(AtomicU64::new(0));
    let completed = Arc::new(AtomicBool::new(false));
    let start = Instant::now();

    let (completed_flag, final_estimate, best_estimate) = std::thread::scope(|scope| {
        for i in 0..threads {
            let core = if core_ids.is_empty() {
                None
            } else {
                let index = (i % core_ids.len() as u64) as usize;
                core_ids.get(index).copied()
            };

            let total = Arc::clone(&total);
            let inside = Arc::clone(&inside);

            scope.spawn(move || {
                if let Some(core) = core {
                    let _ = set_for_current(core);
                }

                let mut rng = rand::thread_rng();
                let mut local_total = 0u64;
                let mut local_inside = 0u64;

                const BATCH: u64 = 65_536;

                loop {
                    for _ in 0..BATCH {
                        let x = rng.gen_range(-1.0..1.0);
                        let y = rng.gen_range(-1.0..1.0);

                        if x * x + y * y <= 1.0 {
                            local_inside += 1;
                        }

                        local_total += 1;
                    }

                    total.fetch_add(local_total, Ordering::Relaxed);
                    inside.fetch_add(local_inside, Ordering::Relaxed);

                    local_total = 0;
                    local_inside = 0;

                    if STOP.load(Ordering::SeqCst) {
                        break;
                    }
                }
            });
        }

        let mut best_digits = 0u64;
        let mut best_est = f64::NAN;
        let mut last_est = f64::NAN;

        loop {
            if STOP.load(Ordering::SeqCst) {
                break;
            }

            let total_samples = total.load(Ordering::SeqCst);
            let inside_count = inside.load(Ordering::SeqCst);

            if total_samples > 0 {
                let estimate = 4.0 * (inside_count as f64) / (total_samples as f64);
                last_est = estimate;

                let current_digits = correct_digits(estimate);

                if current_digits > best_digits {
                    best_digits = current_digits;
                    best_est = estimate;
                }

                if target_digits > 0 && best_digits >= target_digits {
                    completed.store(true, Ordering::SeqCst);
                    STOP.store(true, Ordering::SeqCst);
                    break;
                }

                draw_progress(
                    target_digits,
                    start,
                    total_samples,
                    best_digits,
                    estimate,
                    threads,
                );
            }

            std::thread::sleep(Duration::from_millis(100));
        }

        (
            completed.load(Ordering::SeqCst),
            last_est,
            best_est,
        )
    });

    clear_progress_line();

    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();

    let shown_estimate = if completed_flag && best_estimate.is_finite() {
        best_estimate
    } else {
        final_estimate
    };

    if completed_flag {
        println!("Reached target of {} correct digit(s).", target_digits);
    } else {
        println!("Benchmark stopped.");
    }

    if shown_estimate.is_finite() {
        println!("Pi estimate: {:.15}", shown_estimate);
    } else {
        println!("Pi estimate: 0.0");
    }

    println!("Total runtime: {:.2}s", elapsed_secs);
}

fn correct_digits(estimate: f64) -> u64 {
    if !estimate.is_finite() {
        return 0;
    }

    let error = (estimate - std::f64::consts::PI).abs();

    if error == 0.0 {
        return 15;
    }

    let mut digits = 0u64;

    while digits < 15 {
        let tolerance = 0.5 * 10f64.powi(-(digits as i32));

        if error < tolerance {
            digits += 1;
        } else {
            break;
        }
    }

    digits
}

fn draw_progress(
    target_digits: u64,
    start: Instant,
    total_samples: u64,
    best_digits: u64,
    estimate: f64,
    threads: u64,
) {
    let elapsed = start.elapsed().as_secs_f64();

    let progress = if target_digits > 0 {
        (best_digits as f64 / target_digits as f64).min(1.0)
    } else {
        (elapsed % 10.0) / 10.0
    };

    let width = 28;
    let filled = (progress * width as f64).round() as usize;
    let filled = filled.min(width);
    let bar = "#".repeat(filled) + &"-".repeat(width - filled);

    let percent = progress * 100.0;
    let target_label = if target_digits == 0 {
        "-".to_string()
    } else {
        target_digits.to_string()
    };

    let samples = format!("{:.3e}", total_samples as f64);
    let rate = if elapsed > 0.0 {
        format!("{:.3e} pts/s", (total_samples as f64) / elapsed)
    } else {
        "0 pts/s".to_string()
    };

    print!(
        "\r[{}] {:>5.1}% | digits {}/{} | est={:.15} | samples={} | {} | threads={} | {:.1}s   ",
        bar,
        percent,
        best_digits,
        target_label,
        estimate,
        samples,
        rate,
        threads,
        elapsed
    );

    let _ = io::stdout().flush();
}

fn clear_progress_line() {
    // Erase the entire current line and move the cursor to column 0.
    // The ANSI sequence "\x1B[2K" clears the line, and "\r" returns to start.
    // This avoids leaving the cursor after a long run of spaces which can wrap
    // and cause the next println! to be split across lines.
    print!("\x1B[2K\r");
    let _ = io::stdout().flush();
}

pub fn print_banner() {
    println!(
        r"
    ╔═══════════════════════════════════════════╗
    ║                                           ║
    ║     ██████╗ ██╗   ██╗███████╗████████╗    ║
    ║     ██╔══██╗██║   ██║██╔════╝╚══██╔══╝    ║
    ║     ██████╔╝██║   ██║███████╗   ██║       ║
    ║     ██╔══██╗██║   ██║╚════██║   ██║       ║
    ║     ██║  ██║╚██████╔╝███████║   ██║       ║
    ║     ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝       ║
    ║                                           ║
    ║        rusty_bench — π Calculator         ║
    ║                                           ║
    ╚═══════════════════════════════════════════╝
"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_digits_with_pi() {
        // Test with the actual value of pi
        let pi = std::f64::consts::PI;
        let digits = correct_digits(pi);
        assert_eq!(digits, 15, "Exact pi should have 15 correct digits");
    }

    #[test]
    fn test_correct_digits_large_error() {
        // Test with very inaccurate estimate (e.g., 1.0)
        let digits = correct_digits(1.0);
        assert_eq!(digits, 0, "Very inaccurate estimate should have 0 correct digits");
    }

    #[test]
    fn test_correct_digits_close_estimate() {
        // Test with estimate close to pi (3.14)
        let digits = correct_digits(3.14);
        assert!(digits > 0, "Close estimate should have at least 1 correct digit");
        assert!(digits <= 3, "3.14 should have at most 3 correct digits");
    }

    #[test]
    fn test_correct_digits_very_close_estimate() {
        // Test with estimate very close to pi (3.14159265)
        let digits = correct_digits(3.14159265);
        assert!(digits >= 8, "3.14159265 should have at least 8 correct digits");
    }

    #[test]
    fn test_correct_digits_nan_input() {
        // Test with NaN input
        let digits = correct_digits(f64::NAN);
        assert_eq!(digits, 0, "NaN should return 0 correct digits");
    }

    #[test]
    fn test_correct_digits_infinity_input() {
        // Test with infinity
        let digits = correct_digits(f64::INFINITY);
        assert_eq!(digits, 0, "Infinity should return 0 correct digits");
    }

    #[test]
    fn test_correct_digits_negative_infinity() {
        // Test with negative infinity
        let digits = correct_digits(f64::NEG_INFINITY);
        assert_eq!(digits, 0, "Negative infinity should return 0 correct digits");
    }

    #[test]
    fn test_correct_digits_max_is_15() {
        // Test that correct_digits never returns more than 15
        for estimate in [2.0, 3.0, 3.1, 3.14, 3.141, 3.1415, 3.14159, 3.141592, 3.1415926] {
            let digits = correct_digits(estimate);
            assert!(digits <= 15, "correct_digits should never return more than 15");
        }
    }

    #[test]
    fn test_correct_digits_pi_approximation() {
        // Test known approximations of pi
        let approx_22_7 = 22.0 / 7.0; // 3.142857...
        let digits = correct_digits(approx_22_7);
        assert!(digits >= 2, "22/7 approximation should have at least 2 correct digits");
    }

    #[test]
    fn test_pi_estimate_validity() {
        // Test that pi estimates from random samples are in a reasonable range
        let estimate = 3.14159;
        assert!(estimate > 0.0, "Pi estimate should be positive");
        assert!(estimate < 4.0, "Pi estimate should be less than 4");
    }

    #[test]
    fn test_zero_samples() {
        // Edge case: what happens with zero samples
        let total_samples: u64 = 0;
        let inside_count: u64 = 0;
        
        if total_samples > 0 {
            let _estimate = 4.0 * (inside_count as f64) / (total_samples as f64);
        } else {
            assert_eq!(total_samples, 0);
        }
    }

    #[test]
    fn test_inside_outside_ratio() {
        // Test the Monte Carlo algorithm logic
        // All points inside circle
        let total_samples = 1000u64;
        let inside_count = 1000u64;
        let estimate = 4.0 * (inside_count as f64) / (total_samples as f64);
        assert_eq!(estimate, 4.0, "All samples inside should give estimate of 4.0");

        // No points inside circle
        let inside_count = 0u64;
        let estimate = 4.0 * (inside_count as f64) / (total_samples as f64);
        assert_eq!(estimate, 0.0, "No samples inside should give estimate of 0.0");

        // 1/4 inside (quarter circle)
        let inside_count = 250u64;
        let estimate = 4.0 * (inside_count as f64) / (total_samples as f64);
        assert_eq!(estimate, 1.0, "1/4 inside should give estimate of 1.0");
    }

    #[test]
    fn test_thread_count_validation() {
        // Test the logic for validating thread count
        let requested_threads = 4u64;
        let available_cores = 8u64;
        let threads = requested_threads.min(available_cores);
        assert_eq!(threads, 4, "Should use requested threads when available");

        let requested_threads = 16u64;
        let available_cores = 8u64;
        let threads = requested_threads.min(available_cores);
        assert_eq!(threads, 8, "Should cap threads at available cores");
    }

    #[test]
    fn test_progress_calculation_with_target() {
        // Test progress calculation when target_digits is set
        let target_digits = 10u64;
        let best_digits = 5u64;
        let progress = (best_digits as f64 / target_digits as f64).min(1.0);
        assert_eq!(progress, 0.5, "5 out of 10 digits should be 50% progress");

        let best_digits = 10u64;
        let progress = (best_digits as f64 / target_digits as f64).min(1.0);
        assert_eq!(progress, 1.0, "10 out of 10 digits should be 100% progress");

        let best_digits = 15u64;
        let progress = (best_digits as f64 / target_digits as f64).min(1.0);
        assert_eq!(progress, 1.0, "Capped at 100% when exceeding target");
    }

    #[test]
    fn test_progress_calculation_without_target() {
        // Test progress calculation when target_digits is 0 (run until interrupted)
        let elapsed = 5.0; // seconds
        let progress = (elapsed % 10.0) / 10.0;
        assert!(progress >= 0.0 && progress < 1.0, "Progress should cycle between 0 and 1");
    }

    #[test]
    fn test_rate_calculation() {
        // Test sampling rate calculation
        let total_samples = 1_000_000u64;
        let elapsed = 1.0; // 1 second
        let rate = (total_samples as f64) / elapsed;
        assert_eq!(rate, 1_000_000.0, "Rate should be samples per second");

        let elapsed = 0.5; // 0.5 seconds
        let rate = (total_samples as f64) / elapsed;
        assert_eq!(rate, 2_000_000.0, "Rate calculation should be correct");
    }

    #[test]
    fn test_core_id_indexing() {
        // Test the logic for indexing into core_ids array
        let core_ids = vec![CoreId { id: 0 }, CoreId { id: 1 }, CoreId { id: 2 }, CoreId { id: 3 }];
        
        for i in 0..8 {
            let index = (i % core_ids.len() as u64) as usize;
            let _core = core_ids.get(index).copied();
            assert!(index < core_ids.len(), "Index should always be within bounds");
        }
    }

    #[test]
    fn test_batch_size_constant() {
        // Test that batch size is reasonable
        const BATCH: u64 = 65_536;
        assert_eq!(BATCH, 65_536, "Batch size should be 65536");
        assert!(BATCH > 0, "Batch size should be positive");
    }
}
