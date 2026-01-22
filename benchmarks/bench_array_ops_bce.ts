// Benchmark: Array Operations with BCE optimization
// Uses arr.length in loop conditions for bounds check elimination

function runArrayBenchmark(): number {
  const SIZE = 100000;

  // Create array with values (using SIZE for extension)
  const arr: number[] = [];
  for (let i = 0; i < SIZE; i++) {
    arr[i] = i;
  }

  // Sum all elements using arr.length
  let sum = 0;
  for (let i = 0; i < arr.length; i++) {
    sum = sum + arr[i];
  }

  // Reverse array in-place using arr.length
  let left = 0;
  let right = arr.length - 1;
  while (left < right) {
    const temp = arr[left];
    arr[left] = arr[right];
    arr[right] = temp;
    left = left + 1;
    right = right - 1;
  }

  // Count even numbers using arr.length
  let evenCount = 0;
  for (let i = 0; i < arr.length; i++) {
    if (arr[i] % 2 === 0) {
      evenCount = evenCount + 1;
    }
  }

  return sum + evenCount;
}

const WARMUP_ITERATIONS = 5;
const TIMED_ITERATIONS = 100;

// Warmup phase (for JIT fairness)
for (let i = 0; i < WARMUP_ITERATIONS; i++) {
  runArrayBenchmark();
}

// Timed phase
const start = Date.now();
for (let i = 0; i < TIMED_ITERATIONS; i++) {
  runArrayBenchmark();
}
const end = Date.now();

const total = end - start;
const avg = total / TIMED_ITERATIONS;

console.log("BENCHMARK:array_ops_bce");
console.log("TOTAL:" + total);
console.log("ITERATIONS:" + TIMED_ITERATIONS);
console.log("AVG:" + avg);
