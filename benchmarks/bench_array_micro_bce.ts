// Micro-benchmarks with BCE optimization (using arr.length in conditions)
const SIZE = 100000;
const ITERS = 100;

// Pre-allocate array
const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(i);
}

// Test 1: Sequential read with BCE
let start = Date.now();
let sum = 0;
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < arr.length; i++) {
    sum = sum + arr[i];
  }
}
console.log("Sequential read (BCE): " + (Date.now() - start) + "ms");

// Test 2: Sequential write with BCE
start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < arr.length; i++) {
    arr[i] = i;
  }
}
console.log("Sequential write (BCE): " + (Date.now() - start) + "ms");

// Test 3: Push (no bounds check needed)
start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  const newArr: number[] = [];
  for (let i = 0; i < SIZE; i++) {
    newArr.push(i);
  }
}
console.log("Push build: " + (Date.now() - start) + "ms");

console.log("Sum: " + sum);
