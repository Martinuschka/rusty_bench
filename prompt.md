# task
compare different models for execution of given prompt.

# prompt
```
this is an new rust project. I want to create a CLI benchmark based on Rust. It should compute/estimate the Number Pi. In the beginning the user is asked until how many correct digits of Pi the benchmark should be running and how many threads should be used. if the user selects 0 digits, the benchmark would run until cancelled. Depending on given number of threads this amount should be started and calulcate towards Pi at the same time so that each thread can fully run on a dedicated CPU core. The user can interrupt this benchmark at any time on the console and could start all over again beginning with the questions about how many digits of Pi and the number of threads. When the benchmark is running, a nice ASCI-styly progress should be shown in the conosole.
```

# llms
Qwen3.8-27b
