// Benchmark: Recursive Fibonacci
// Measures function call overhead and recursion
function fib(n: number): number {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

const N = 40;
const start = Date.now();
const result = fib(N);
const elapsed = Date.now() - start;

console.log("fibonacci:" + elapsed);
console.log("fib(" + N + "):" + result);
