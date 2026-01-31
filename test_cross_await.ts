/**
 * Test: Cross-module async function with await
 */
import { greet } from './test_module_a';

console.log('Testing cross-module async with await...');

async function main(): Promise<void> {
  console.log('Before await...');
  const result = await greet('World');
  console.log('After await, result:', result);
}

main().then(() => {
  console.log('Done!');
}).catch(e => {
  console.log('Error:', e);
});
