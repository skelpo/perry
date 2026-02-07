// Benchmark: Matrix multiplication using flat arrays
// Measures computed index array access performance
const SIZE = 256;

function matmul(a: number[], b: number[], c: number[], size: number): void {
    for (let i = 0; i < size; i++) {
        for (let j = 0; j < size; j++) {
            let sum = 0;
            for (let k = 0; k < size; k++) {
                sum = sum + a[i * size + k] * b[k * size + j];
            }
            c[i * size + j] = sum;
        }
    }
}

// Initialize matrices
const n = SIZE * SIZE;
const a: number[] = [];
const b: number[] = [];
const c: number[] = [];
for (let i = 0; i < n; i++) {
    a.push(i % 100);
    b.push(i % 100);
    c.push(0);
}

const start = Date.now();
matmul(a, b, c, SIZE);
const elapsed = Date.now() - start;

// Checksum
let checksum = 0;
for (let i = 0; i < n; i++) {
    checksum = checksum + c[i];
}

console.log("matrix_multiply:" + elapsed);
console.log("checksum:" + checksum);
