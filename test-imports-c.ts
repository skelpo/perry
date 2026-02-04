import { CHAIN_NAMES, NUMBERS } from './test-module-c.js';

console.log('Chains:', CHAIN_NAMES.join(', '));
console.log('Chains length:', CHAIN_NAMES.length);

console.log('Numbers:', NUMBERS.join(', '));

// Test map on imported array
const doubled = NUMBERS.map(n => n * 2);
console.log('Doubled:', doubled.join(', '));

// Test filter on imported array
const evens = NUMBERS.filter(n => n % 2 === 0);
console.log('Evens:', evens.join(', '));

// Test reduce on imported array
const sum = NUMBERS.reduce((acc, n) => acc + n, 0);
console.log('Sum:', sum);

// Test forEach on imported array
let total = 0;
NUMBERS.forEach(n => {
    total = total + n;
});
console.log('Total:', total);
