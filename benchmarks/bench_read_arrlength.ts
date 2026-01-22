// Standalone read benchmark with arr.length (BCE)
const SIZE = 100000;
const ITERS = 100;

const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(i);
}

// Single test: Sequential read with arr.length
const start = Date.now();
let sum = 0;
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < arr.length; i++) {
    sum = sum + arr[i];
  }
}
console.log("Read (arr.length): " + (Date.now() - start) + "ms");
console.log("Sum: " + sum);
