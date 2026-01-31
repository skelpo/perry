/**
 * Test: Nested async with "response" variable name
 */

async function innerFetch(url: string): Promise<string> {
  const response = await fetch(url);  // Uses "response" internally
  const data = await response.json();
  return JSON.stringify(data);
}

async function callAPI(query: string): Promise<string> {
  const result = await innerFetch('https://httpbin.org/anything?q=' + query);
  // Parse and return a portion
  const parsed = JSON.parse(result);
  if (parsed.args && parsed.args.q) {
    return parsed.args.q;
  }
  return 'no result';
}

async function main(): Promise<void> {
  // Test with "result" - should work
  const result = await callAPI('hello');
  console.log('result:', result);
  console.log('result.length:', result.length);

  // Test with "response" - might be broken
  const response = await callAPI('world');
  console.log('response:', response);
  console.log('response.length:', response.length);
}

main().catch(e => console.error('Error:', e));
