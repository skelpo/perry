// Micro-benchmark: Array read-only operations

function runReadBenchmark(): number {
  const SIZE = 100000;

  // Pre-allocate array with push (known to be fast)
  const arr: number[] = [];
  for (let i = 0; i < SIZE; i++) {
    arr.push(i);
  }

  // Now just read operations (1000 iterations to amplify)
  let sum = 0;
  for (let iter = 0; iter < 100; iter++) {
    for (let i = 0; i < SIZE; i++) {
      sum = sum + arr[i];
    }
  }

  return sum;
}

const start = Date.now();
runReadBenchmark();
const end = Date.now();

console.log("Read-only time: " + (end - start) + "ms");
