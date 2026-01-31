function testTrim(value: string): void {
  console.log('1. value:', value);
  console.log('2. value.length:', value.length);
  const trimmed = value.trim();
  console.log('3. trimmed:', trimmed);
  console.log('4. trimmed.length:', trimmed.length);
  console.log('5. Concat test:', 'X' + trimmed + 'X');
}

testTrim("  Hello  ");
console.log('Done');
