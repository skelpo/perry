// Benchmark: Integer Arithmetic Operations
// Tests: tight loops, register operations

function runArithmeticBenchmark(): number {
  const ITERATIONS = 10000000;
  let result = 0;
  let a = 12345678;
  let b = 87654321;

  for (let i = 0; i < ITERATIONS; i++) {
    // Mix of arithmetic operations
    result = result + (a % 1000);
    result = result - (b % 1000);
    result = result + ((a * 3) % 10000);
    result = result - ((b * 2) % 10000);

    // Update a and b
    a = a + 1;
    b = b - 1;

    // Keep values bounded
    if (a > 100000000) {
      a = 12345678;
    }
    if (b < 0) {
      b = 87654321;
    }
  }

  return result;
}

const WARMUP_ITERATIONS = 5;
const TIMED_ITERATIONS = 100;

// Warmup phase (for JIT fairness)
for (let i = 0; i < WARMUP_ITERATIONS; i++) {
  runArithmeticBenchmark();
}

// Timed phase
const start = Date.now();
for (let i = 0; i < TIMED_ITERATIONS; i++) {
  runArithmeticBenchmark();
}
const end = Date.now();

const total = end - start;
const avg = total / TIMED_ITERATIONS;

console.log("BENCHMARK:arithmetic");
console.log("TOTAL:" + total);
console.log("ITERATIONS:" + TIMED_ITERATIONS);
console.log("AVG:" + avg);
