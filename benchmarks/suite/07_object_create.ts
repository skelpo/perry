// Benchmark: Object creation and field access
// Measures object allocation overhead
const ITERATIONS = 1000000;

class Point {
    x: number;
    y: number;
    constructor(x: number, y: number) {
        this.x = x;
        this.y = y;
    }
}

let sum = 0;
const start = Date.now();
for (let i = 0; i < ITERATIONS; i++) {
    const p = new Point(i, i + 1);
    sum = sum + p.x + p.y;
}
const elapsed = Date.now() - start;

console.log("object_create:" + elapsed);
console.log("sum:" + sum);
