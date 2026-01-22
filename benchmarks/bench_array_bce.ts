// Benchmark: bounds check elimination test
// Uses arr.length in condition (not a constant) to enable BCE optimization
const SIZE = 100000;
const ITERS = 100;

// Pre-allocate with push
const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(0);
}

// Test writes with arr.length condition (enables BCE)
const start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < arr.length; i++) {
    arr[i] = i;
  }
}
const elapsed = Date.now() - start;
console.log("BCE write: " + elapsed + "ms");
console.log("Per iteration: " + (elapsed / ITERS) + "ms");
