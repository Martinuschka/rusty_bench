# rusty_bench - π Calculator Benchmark

A Rust-based Monte Carlo simulation benchmark for estimating the value of π using multi-threading with CPU core pinning.
Implementation was done using different LLMs in comparison (individual branches):
[prompt.md](prompt.md)

## Features
- **Monte Carlo π estimation** using random point generation
- **Thread pinning** to specific CPU cores via `core_affinity` crate
- Real-time progress visualization with:
   - Accuracy percentage
   - Sampling rate
   - Current estimate of π
   - Thread utilization information
- Graceful shutdown via Ctrl+C
- Input validation for target digits (0-15) and thread counts
- Comprehensive test suite covering edge cases and core functionality

## Usage

To run the benchmark:
```shell script
cargo run --release
```


### Interaction Guide
1. When prompted, enter:
   - Number of correct π digits to calculate (0 = unlimited)
   - Number of threads to use (auto-limited by available CPU cores)
2. Press Enter to start
3. Use Ctrl+C at any time to stop the calculation

Example session:
```
=== Pi benchmark ===
Enter 0 digits to run until interrupted. Enter q at a prompt to quit.
How many correct digits of Pi? (0 = run until Ctrl+C, q = quit): 5
How many threads should be used? (q = quit): 4
[############################] 100.0% | digits 5/5 | est=3.14159 | samples=1.23e+06 | 1.23e+05 pts/s | threads=4 | 10.23s
Reached target of 5 correct digit(s).
Pi estimate: 3.14159
Total runtime: 10.23s
```


## Dependencies

The project requires the following Rust crates:
- `core_affinity` For CPU core pinning
- `ctrlc` For handling Ctrl+C interrupts
- `rand` For random number generation

These dependencies are automatically managed by Cargo.

## Notes
- The program uses a batch size of 65,536 points per thread iteration for performance
- Accuracy is calculated using a tolerance-based comparison against π
- If core affinity enumeration fails, threads will not be pinned but will still run in parallel
- The maximum achievable precision with f64 is approximately 15 digits
