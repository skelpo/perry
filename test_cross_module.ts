/**
 * Test: Cross-module function imports
 */
import { add, greet } from './test_module_a';

console.log('Testing cross-module imports...');
console.log('typeof add:', typeof add);
console.log('typeof greet:', typeof greet);

// Try calling them
const sum = add(2, 3);
console.log('add(2, 3) =', sum);

async function main(): Promise<void> {
  const greeting = await greet('World');
  console.log('greet("World") =', greeting);
}

main().catch(e => console.error('Error:', e));
