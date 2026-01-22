// Benchmark: Array read performance
// Measures sequential array read with accumulation
const SIZE = 10000000;
const arr: number[] = [];
for (let i = 0; i < SIZE; i++) {
    arr[i] = i;
}

let sum = 0;
const start = Date.now();
for (let i = 0; i < arr.length; i++) {
    sum = sum + arr[i];
}
const elapsed = Date.now() - start;

console.log("array_read:" + elapsed);
console.log("sum:" + sum);
