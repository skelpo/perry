// Benchmark: Array write performance
// Measures sequential array write with bounds checking
const SIZE = 10000000;
const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
    arr[i] = 0;
}

const start = Date.now();
for (let i = 0; i < arr.length; i++) {
    arr[i] = i;
}
const elapsed = Date.now() - start;

console.log("array_write:" + elapsed);
console.log("checksum:" + arr[SIZE - 1]);
