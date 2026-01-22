// Test with manually inlined code to compare
const ITERS = 100;
let sum = 0;

const start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
    sum = 0;
    for (let i = 0; i < 1000000; i++) {
        // Manually inlined: sum = add(sum, square(i % 10))
        const x = i % 10;
        sum = sum + (x * x);
    }
}
console.log("Manual inline: " + (Date.now() - start) + "ms");
console.log("Sum: " + sum);
