// Test returning deeply nested JSON value from async function

async function fetchDeepNested(): Promise<string> {
  console.log('1. In fetchDeepNested');

  const response = await fetch('https://httpbin.org/anything', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      choices: [
        {
          message: {
            content: 'This is the deeply nested content value'
          }
        }
      ]
    }),
  });

  console.log('2. Got response');
  const data = await response.json();
  console.log('3. Parsed JSON');

  // Mirror OpenAI's response structure: data.json.choices[0].message.content
  console.log('4. data.json exists:', !!data.json);
  if (data.json) {
    console.log('5. data.json.choices exists:', !!data.json.choices);
    if (data.json.choices) {
      console.log('6. choices length:', data.json.choices.length);
      if (data.json.choices.length > 0) {
        const firstChoice = data.json.choices[0];
        console.log('7. firstChoice type:', typeof firstChoice);
        console.log('8. firstChoice.message exists:', !!firstChoice.message);
        if (firstChoice.message) {
          const message = firstChoice.message;
          console.log('9. message type:', typeof message);
          console.log('10. message.content exists:', !!message.content);
          if (message.content) {
            const content = message.content;
            console.log('11. content type:', typeof content);
            console.log('12. About to return content');
            return content;
          }
        }
      }
    }
  }
  return '';
}

async function main(): Promise<void> {
  console.log('Starting test...');
  const result = await fetchDeepNested();
  console.log('13. Got result back:', result);
  console.log('Done');
}

main().catch(e => console.error('Error:', e));
