/**
 * Test: Variable named "response" from async function with JSON
 * Simulates the OpenAI pattern
 */

async function callOpenAI(query: string): Promise<string> {
  const resp = await fetch('https://httpbin.org/anything', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      model: 'gpt-4',
      messages: [{ role: 'user', content: query }]
    })
  });

  const data = await resp.json();

  // Simulate OpenAI response structure
  if (data.json && data.json.messages && data.json.messages.length > 0) {
    return data.json.messages[0].content;
  }
  return 'fallback response';
}

async function main(): Promise<void> {
  console.log('Testing with result...');
  const result = await callOpenAI('test query');
  console.log('result:', result);
  console.log('result.length:', result.length);

  console.log('Testing with response...');
  const response = await callOpenAI('test query');
  console.log('response:', response);
  console.log('response.length:', response.length);
}

main().catch(e => console.error('Error:', e));
