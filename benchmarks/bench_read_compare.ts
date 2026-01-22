// Compare read performance: constant vs arr.length
const SIZE = 100000;
const ITERS = 100;

const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(i);
}

// Test 1: Using constant
let start = Date.now();
let sum = 0;
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < SIZE; i++) {
    sum = sum + arr[i];
  }
}
console.log("With constant: " + (Date.now() - start) + "ms");

// Test 2: Using arr.length
start = Date.now();
let sum2 = 0;
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < arr.length; i++) {
    sum2 = sum2 + arr[i];
  }
}
console.log("With arr.length: " + (Date.now() - start) + "ms");

console.log("Sum: " + sum + ", " + sum2);
