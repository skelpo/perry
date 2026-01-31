/**
 * Test: Cross-module string function import
 */
import { greetSync, addStr } from './test_module_sync';

console.log('Testing cross-module string imports...');
console.log('typeof greetSync:', typeof greetSync);
console.log('typeof addStr:', typeof addStr);

const greeting = greetSync('World');
console.log('greetSync("World") =', greeting);

const combined = addStr('Hello, ', 'Perry');
console.log('addStr("Hello, ", "Perry") =', combined);

console.log('All string tests passed!');
