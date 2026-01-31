import { execSync } from 'child_process';

// Test execSync with toString
const result = execSync('echo hello');
const str = result.toString();
console.log('execSync result:', str);
