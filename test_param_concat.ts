/**
 * Test: String concat with parameter substring
 * Simulates the mysql param pattern without requiring mysql
 */

function useWithConcat(value: string): void {
  console.log('1. Got value type:', typeof value);
  console.log('2. Value:', value);

  // Test substring
  console.log('3. Testing substring...');
  const sub = value.substring(0, 20);
  console.log('4. Substring result:', sub);

  // Test concat - this is where SIGSEGV happens with mysql strings
  console.log('5. Testing concat...');
  const concat = 'Response for: ' + sub;
  console.log('6. Concat result:', concat);

  console.log('7. Done');
}

async function main(): Promise<void> {
  // Simulate a string from JSON (similar to mysql result)
  const jsonStr = JSON.stringify({ queryText: 'This is a test query from the database' });
  const parsed = JSON.parse(jsonStr);
  const queryText = parsed.queryText;

  console.log('Got from JSON');
  useWithConcat(queryText);
  console.log('Done');
}

main().catch(e => console.error('Error:', e));
