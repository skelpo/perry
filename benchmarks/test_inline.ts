// Test function inlining optimization
// Helper function - should be inlined
function add(a: number, b: number): number {
    return a + b;
}

function square(x: number): number {
    return x * x;
}

// Main code - calls should be inlined
const ITERS = 100;
let sum = 0;

const start = Date.now();
for (let iter = 0; iter < ITERS; iter++) {
    sum = 0;
    for (let i = 0; i < 1000000; i++) {
        sum = add(sum, square(i % 10));
    }
}
console.log("Inline test: " + (Date.now() - start) + "ms");
console.log("Sum: " + sum);
