// Test all string methods on function parameters

function testStringMethods(value: string): void {
  console.log('Testing string methods:');
  console.log('1. typeof:', typeof value);
  console.log('2. value:', value);
  console.log('3. length:', value.length);
  console.log('4. substring(0, 5):', value.substring(0, 5));
  console.log('5. slice(0, 5):', value.slice(0, 5));
  console.log('6. slice(-5):', value.slice(-5));
  console.log('7. value[0]:', value[0]);
  console.log('8. value[4]:', value[4]);
  console.log('9. toLowerCase():', value.toLowerCase());
  console.log('10. toUpperCase():', value.toUpperCase());
  console.log('11. trim():', '  ' + value.trim() + '  ');
}

// Test with a normal string
const str = "Hello World!";
console.log('=== Testing with literal string ===');
testStringMethods(str);

// Test passing a string through another function
function getAnotherString(): string {
  return "TypeScript Compiler";
}

console.log('\n=== Testing with function return value ===');
testStringMethods(getAnotherString());

// Test with concatenated string
console.log('\n=== Testing with concatenated string ===');
testStringMethods("Foo" + " Bar" + " Baz");

console.log('\nAll tests complete!');
