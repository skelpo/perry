// Test large string async return

async function getLargeString(size: number): Promise<string> {
  console.log('1. Creating string of size:', size);
  const result = 'A'.repeat(size);
  console.log('2. Created string, length:', result.length);
  console.log('3. First 10 chars:', result.substring(0, 10));
  console.log('4. About to return');
  return result;
}

async function main(): Promise<void> {
  console.log('=== Testing small string (100 chars) ===');
  const small = await getLargeString(100);
  console.log('5. Got result, length:', small.length);
  console.log('6. First 10 chars:', small.substring(0, 10));

  console.log('\n=== Testing medium string (500 chars) ===');
  const medium = await getLargeString(500);
  console.log('5. Got result, length:', medium.length);
  console.log('6. First 10 chars:', medium.substring(0, 10));

  console.log('\n=== Testing large string (2000 chars) ===');
  const large = await getLargeString(2000);
  console.log('5. Got result, length:', large.length);
  console.log('6. First 10 chars:', large.substring(0, 10));

  console.log('\n=== Testing very large string (5000 chars) ===');
  const veryLarge = await getLargeString(5000);
  console.log('5. Got result, length:', veryLarge.length);
  console.log('6. First 10 chars:', veryLarge.substring(0, 10));

  console.log('\nDone');
}

main().catch(e => console.error('Error:', e));
