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

    println!("=== Pi benchmark ===");
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
    print!("\r{}   ", " ".repeat(200));
    let _ = io::stdout().flush();
}