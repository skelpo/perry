// Test returning from deeply nested if-blocks in async functions

async function fetchData(): Promise<string> {
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

  const data = await response.json();

  // 4+ levels of nested if-blocks (like OpenAI response handling)
  console.log('1. data.json exists:', !!data.json);
  if (data.json) {
    console.log('2. data.json.choices exists:', !!data.json.choices);
    if (data.json.choices) {
      console.log('3. choices.length:', data.json.choices.length);
      if (data.json.choices.length > 0) {
        const firstChoice = data.json.choices[0];
        console.log('4. firstChoice exists:', !!firstChoice);
        if (firstChoice) {
          console.log('5. firstChoice.message exists:', !!firstChoice.message);
          if (firstChoice.message) {
            const message = firstChoice.message;
            console.log('6. message.content exists:', !!message.content);
            if (message.content) {
              const content = message.content;
              console.log('7. content type:', typeof content);
              console.log('8. content value:', content);
              console.log('9. About to return from level 6');
              return content;
            }
          }
        }
      }
    }
  }
  return 'fallback';
}

async function main(): Promise<void> {
  console.log('Starting...');
  const result = await fetchData();
  console.log('10. Got result:', result);
  console.log('11. Result type:', typeof result);
  console.log('Done');
}

main().catch(e => console.error('Error:', e));
