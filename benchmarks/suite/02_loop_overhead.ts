// Benchmark: Loop overhead
// Measures raw loop iteration speed without array access
const ITERATIONS = 100000000;
let sum = 0;

const start = Date.now();
for (let i = 0; i < ITERATIONS; i++) {
    sum = sum + 1;
}
const elapsed = Date.now() - start;

console.log("loop_overhead:" + elapsed);
console.log("sum:" + sum);
