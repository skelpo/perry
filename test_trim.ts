function testTrim(value: string): void {
  console.log('1. value:', value);
  console.log('2. About to trim...');
  const trimmed = value.trim();
  console.log('3. trim result:', trimmed);
}

testTrim("  Hello  ");
console.log('Done');
