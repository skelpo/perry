// Benchmark: Large number computation
// Measures numeric computation with overflow handling
const ITERATIONS = 100000000;
let sum = 0;

const start = Date.now();
for (let i = 0; i < ITERATIONS; i++) {
    // Simulate factorial-like accumulation pattern
    sum = sum + (i % 1000);
}
const elapsed = Date.now() - start;

console.log("accumulate:" + elapsed);
console.log("sum:" + sum);
