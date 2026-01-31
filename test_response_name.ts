/**
 * Test: Variable named "response" from async function
 * Bug: Using "response" as variable name causes value corruption
 */

async function getString(): Promise<string> {
  return "hello world";
}

async function main(): Promise<void> {
  // This should work
  const result = await getString();
  console.log('result:', result);
  console.log('result.length:', result.length);

  // This might be broken
  const response = await getString();
  console.log('response:', response);
  console.log('response.length:', response.length);
}

main().catch(e => console.error('Error:', e));
