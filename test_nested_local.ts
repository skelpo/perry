// Test deeply nested if-blocks with JSON.parse (no network)
async function fetchData(): Promise<string> {
  // Simulate the nested JSON structure from OpenAI response
  const jsonStr = JSON.stringify({
    choices: [
      {
        message: {
          content: 'This is the deeply nested content value'
        }
      }
    ]
  });

  const data = JSON.parse(jsonStr);

  // 4+ levels of nested if-blocks (like OpenAI response handling)
  console.log('1. data exists:', !!data);
  if (data) {
    console.log('2. data.choices exists:', !!data.choices);
    if (data.choices) {
      console.log('3. choices.length:', data.choices.length);
      if (data.choices.length > 0) {
        const firstChoice = data.choices[0];
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
