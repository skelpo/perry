// Micro-benchmarks to isolate array bottlenecks
const SIZE = 100000;
const ITERS = 100;

// Pre-allocate array
const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(i);
}

// Test 1: Sequential read (best case for bounds check elimination)
let start = Date.now();
let sum = 0;
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < SIZE; i++) {
    sum = sum + arr[i];
  }
}
console.log("Sequential read: " + (Date.now() - start) + "ms");

// Test 2: Sequential write (in-bounds)
start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < SIZE; i++) {
    arr[i] = i;
  }
}
console.log("Sequential write: " + (Date.now() - start) + "ms");

// Test 3: Random access pattern
const indices: number[] = [];
for (let i = 0; i < SIZE; i++) {
  indices.push(Math.floor(Math.random() * SIZE));
}
start = Date.now();
sum = 0;
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < SIZE; i++) {
    sum = sum + arr[indices[i]];
  }
}
console.log("Random read: " + (Date.now() - start) + "ms");

// Test 4: Push (no bounds check needed)
start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  const newArr: number[] = [];
  for (let i = 0; i < SIZE; i++) {
    newArr.push(i);
  }
}
console.log("Push build: " + (Date.now() - start) + "ms");

console.log("Sum: " + sum);
