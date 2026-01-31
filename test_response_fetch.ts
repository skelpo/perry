/**
 * Test: Variable named "response" from fetch
 */

async function callAPI(url: string): Promise<string> {
  const resp = await fetch(url);
  const data = await resp.json();
  return data.url || "no url";
}

async function main(): Promise<void> {
  // Using "result" as variable name
  const result = await callAPI('https://httpbin.org/get');
  console.log('result:', result);
  console.log('result.length:', result.length);

  // Using "response" as variable name
  const response = await callAPI('https://httpbin.org/get');
  console.log('response:', response);
  console.log('response.length:', response.length);
}

main().catch(e => console.error('Error:', e));
