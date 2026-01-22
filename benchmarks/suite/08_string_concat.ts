// Benchmark: String concatenation
// Measures string allocation and concatenation
const ITERATIONS = 100000;

let result = "";
const start = Date.now();
for (let i = 0; i < ITERATIONS; i++) {
    result = result + "x";
}
const elapsed = Date.now() - start;

console.log("string_concat:" + elapsed);
console.log("length:" + result.length);
