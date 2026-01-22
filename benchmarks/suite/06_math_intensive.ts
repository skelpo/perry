// Benchmark: Math-intensive computation
// Measures floating point operations
const ITERATIONS = 50000000;
let result = 1.0;

const start = Date.now();
for (let i = 1; i < ITERATIONS; i++) {
    result = result + (1.0 / i);
}
const elapsed = Date.now() - start;

console.log("math_intensive:" + elapsed);
console.log("result:" + result);
