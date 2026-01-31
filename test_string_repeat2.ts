// Test string.repeat() more specifically

// Test 1: Local variable from literal
const a = 'A';
console.log("1. a.repeat(5):", a.repeat(5));

// Test 2: Direct literal
console.log("2. 'B'.repeat(5):", 'B'.repeat(5));

// Test 3: Function parameter
function repeatParam(s: string): string {
  return s.repeat(3);
}
console.log("3. repeatParam('C'):", repeatParam('C'));

// Test 4: Return from another function
function getStr(): string {
  return 'D';
}
console.log("4. getStr().repeat(3):", getStr().repeat(3));

console.log('Done');
