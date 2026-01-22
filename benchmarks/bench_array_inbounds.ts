// Benchmark: in-bounds writes to existing array
const SIZE = 100000;
const ITERS = 100;

// Pre-allocate with push
const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
  arr.push(0);
}

// Now test in-bounds writes (arr already has SIZE elements)
const start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
  for (let i = 0; i < SIZE; i++) {
    arr[i] = i;
  }
}
const elapsed = Date.now() - start;
console.log("In-bounds write: " + elapsed + "ms");
console.log("Per iteration: " + (elapsed / ITERS) + "ms");
