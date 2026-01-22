// Micro-benchmark: Array swap operations (like in reverse)

function runSwapBenchmark(): number {
  const SIZE = 100000;

  // Pre-allocate array
  const arr: number[] = [];
  for (let i = 0; i < SIZE; i++) {
    arr.push(i);
  }

  // Swap operations (like in reverse) - 100 iterations
  for (let iter = 0; iter < 100; iter++) {
    let left = 0;
    let right = SIZE - 1;
    while (left < right) {
      const temp = arr[left];
      arr[left] = arr[right];
      arr[right] = temp;
      left = left + 1;
      right = right - 1;
    }
  }

  return arr[0];
}

const start = Date.now();
runSwapBenchmark();
const end = Date.now();

console.log("Swap time: " + (end - start) + "ms");
