// Standalone write benchmark with arr.length (BCE)
const SIZE = 100000;
const ITERS = 100;

const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(0);
}

// Single test: Sequential write with arr.length
const start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < arr.length; i++) {
    arr[i] = i;
  }
}
console.log("Write (arr.length): " + (Date.now() - start) + "ms");
