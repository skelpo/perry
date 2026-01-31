/**
 * Debug: Cross-module async function
 */
import { greet } from './test_module_a';

console.log('Debug test...');

async function main(): Promise<void> {
  console.log('Calling greet...');
  const result = await greet('World');
  console.log('Raw result type:', typeof result);
  console.log('Result:', result);
  // Test if we can use it as a string
  if (typeof result === 'string') {
    console.log('Length:', result.length);
  }
}

main().then(() => {
  console.log('Success!');
}).catch(e => {
  console.log('Error:', e);
});
