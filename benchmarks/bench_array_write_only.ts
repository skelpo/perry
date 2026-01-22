// Micro-benchmark: Array write-only operations

function runWriteBenchmark(): number {
  const SIZE = 100000;

  // Test 1: Build array with indexed writes (extend pattern)
  let sum = 0;
  for (let iter = 0; iter < 100; iter++) {
    const arr: number[] = [];
    for (let i = 0; i < SIZE; i++) {
      arr[i] = i;
    }
    sum = sum + arr[0];
  }

  return sum;
}

const start = Date.now();
runWriteBenchmark();
const end = Date.now();

console.log("Write-only (extend) time: " + (end - start) + "ms");
