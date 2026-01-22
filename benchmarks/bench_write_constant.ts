// Write benchmark with constant value
const SIZE = 100000;
const ITERS = 100;

const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(0);
}

const start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < arr.length; i++) {
    arr[i] = 42;
  }
}
console.log("Write (constant): " + (Date.now() - start) + "ms");
