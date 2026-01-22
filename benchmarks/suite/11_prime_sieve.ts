// Benchmark: Sieve of Eratosthenes
// Classic algorithm benchmark - array access + conditionals
const LIMIT = 1000000;
const sieve: boolean[] = [];

// Initialize all to true (prime candidate)
for (let i = 0; i < LIMIT; i++) {
    sieve[i] = true;
}
sieve[0] = false;
sieve[1] = false;

const start = Date.now();
for (let i = 2; i * i < LIMIT; i++) {
    if (sieve[i]) {
        for (let j = i * i; j < LIMIT; j = j + i) {
            sieve[j] = false;
        }
    }
}

// Count primes
let count = 0;
for (let i = 0; i < LIMIT; i++) {
    if (sieve[i]) {
        count = count + 1;
    }
}
const elapsed = Date.now() - start;

console.log("prime_sieve:" + elapsed);
console.log("primes:" + count);
