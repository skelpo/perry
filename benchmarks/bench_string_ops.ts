// Benchmark: String Operations
// Tests: heap allocation, string methods

function runStringBenchmark(): number {
  const SIZE = 10000;

  // Build string via concatenation
  let str = "";
  for (let i = 0; i < SIZE; i++) {
    str = str + "a";
  }

  // Perform indexOf lookups
  let foundCount = 0;
  const patterns = ["aaa", "aaaa", "aaaaa"];
  for (let p = 0; p < patterns.length; p++) {
    let idx = 0;
    while (idx < str.length) {
      const found = str.indexOf(patterns[p], idx);
      if (found === -1) {
        break;
      }
      foundCount = foundCount + 1;
      idx = found + 1;
    }
  }

  // Perform slice operations
  let sliceSum = 0;
  for (let i = 0; i < 1000; i++) {
    const start = i % (SIZE - 100);
    const slice = str.slice(start, start + 100);
    sliceSum = sliceSum + slice.length;
  }

  return foundCount + sliceSum;
}

const WARMUP_ITERATIONS = 5;
const TIMED_ITERATIONS = 100;

// Warmup phase (for JIT fairness)
for (let i = 0; i < WARMUP_ITERATIONS; i++) {
  runStringBenchmark();
}

// Timed phase
const start = Date.now();
for (let i = 0; i < TIMED_ITERATIONS; i++) {
  runStringBenchmark();
}
const end = Date.now();

const total = end - start;
const avg = total / TIMED_ITERATIONS;

console.log("BENCHMARK:string_ops");
console.log("TOTAL:" + total);
console.log("ITERATIONS:" + TIMED_ITERATIONS);
console.log("AVG:" + avg);
