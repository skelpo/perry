// Test string slice and index access on function parameters

function testString(value: string): void {
  console.log('Testing string methods on parameter:');
  console.log('1. typeof value:', typeof value);
  console.log('2. value:', value);
  console.log('3. value.length:', value.length);
  console.log('4. value.substring(0, 5):', value.substring(0, 5));
  console.log('5. value.slice(0, 5):', value.slice(0, 5));
  console.log('6. value[0]:', value[0]);
}

const str = "Hello World";
console.log('Original string:', str);
testString(str);
console.log('Done');
