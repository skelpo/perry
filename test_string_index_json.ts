// Test string index access on values from JSON

async function main(): Promise<void> {
  console.log('=== Test 1: String index on local variable ===');
  const localStr = 'Hello';
  console.log('localStr[0]:', localStr[0]);

  console.log('\n=== Test 2: String index on JSON.parse result ===');
  const parsed = JSON.parse('{"name": "World"}');
  const name = parsed.name;
  console.log('name:', name);
  console.log('name[0]:', name[0]);

  console.log('\n=== Test 3: String index on fetch JSON result ===');
  const response = await fetch('https://httpbin.org/anything', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ text: 'Testing' }),
  });
  const data = await response.json();
  const text = data.json.text;
  console.log('text:', text);
  console.log('text[0]:', text[0]);

  console.log('\n=== Test 4: Pass JSON string to function ===');
  testStringIndex(text);

  console.log('\nDone');
}

function testStringIndex(s: string): void {
  console.log('In function:');
  console.log('  s:', s);
  console.log('  s[0]:', s[0]);
  console.log('  s.charAt(0):', s.charAt(0));
}

main().catch(e => console.error('Error:', e));
