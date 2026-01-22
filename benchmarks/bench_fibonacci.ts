// Benchmark: Recursive Fibonacci
// Tests: function call overhead, recursion, stack management

function fibonacci(n: number): number {
  if (n <= 1) {
    return n;
  }
  return fibonacci(n - 1) + fibonacci(n - 2);
}

const WARMUP_ITERATIONS = 5;
const TIMED_ITERATIONS = 100;
const FIB_N = 35;

// Warmup phase (for JIT fairness)
for (let i = 0; i < WARMUP_ITERATIONS; i++) {
  fibonacci(FIB_N);
}

// Timed phase
const start = Date.now();
for (let i = 0; i < TIMED_ITERATIONS; i++) {
  fibonacci(FIB_N);
}
const end = Date.now();

const total = end - start;
const avg = total / TIMED_ITERATIONS;

console.log("BENCHMARK:fibonacci");
console.log("TOTAL:" + total);
console.log("ITERATIONS:" + TIMED_ITERATIONS);
console.log("AVG:" + avg);
