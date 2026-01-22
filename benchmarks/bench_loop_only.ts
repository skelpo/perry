// Benchmark loop overhead only (no array access)
const SIZE = 100000;
const ITERS = 100;

const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(0);
}

const start = Date.now();
let sum = 0;
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < arr.length; i++) {
    sum = sum + 1;
  }
}
console.log("Loop only: " + (Date.now() - start) + "ms");
console.log("Sum: " + sum);
