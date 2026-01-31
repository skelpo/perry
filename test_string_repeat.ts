// Test string.repeat()

function testRepeat(s: string): void {
  console.log('1. Input:', s);
  console.log('2. s.repeat(3):', s.repeat(3));
}

// Test with literal
console.log('=== Testing literal ===');
const literal = 'A';
console.log('literal.repeat(10):', literal.repeat(10));

// Test with parameter
console.log('\n=== Testing parameter ===');
testRepeat('X');

// Test longer string
console.log('\n=== Testing longer string ===');
console.log("'Hello'.repeat(3):", 'Hello'.repeat(3));

console.log('\nDone');
