// Benchmark: push vs indexed write
// Push is unchecked (always extends), indexed write has bounds check
const SIZE = 100000;
const ITERS = 100;

// Test push (unchecked)
let start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  const arr: number[] = [];
  for (let i = 0; i < SIZE; i++) {
    arr.push(i);
  }
}
console.log("Push: " + (Date.now() - start) + "ms");

// Test indexed write (checked) - this is slower because it bounds-checks
start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  const arr: number[] = [];
  for (let i = 0; i < SIZE; i++) {
    arr[i] = i;
  }
}
console.log("Indexed: " + (Date.now() - start) + "ms");
