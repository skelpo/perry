/**
 * Test: Cross-module async function import
 */
import { greet } from './test_module_a';

console.log('Testing cross-module async imports...');
console.log('typeof greet:', typeof greet);

// First just call it without await to see if the call works
console.log('Calling greet without await...');
const promise = greet('World');
console.log('Got promise:', promise);
console.log('typeof promise:', typeof promise);
