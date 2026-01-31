/**
 * Test deeply nested JSON access pattern like OpenAI response
 * without network dependencies
 */

async function fetchDeepNested(query: string): Promise<string> {
  console.log('2. In fetchDeepNested with query:', query.substring(0, 50));

  // Mock the httpbin /anything response structure
  const mockHttpbinResponse = {
    json: {
      choices: [
        {
          message: {
            content: 'This is the deeply nested content value'
          }
        }
      ]
    }
  };

  const jsonStr = JSON.stringify(mockHttpbinResponse);
  const data = JSON.parse(jsonStr);

  console.log('3. Got mock response');
  console.log('4. Parsed JSON');

  // Mirror OpenAI's response structure: data.json.choices[0].message.content
  console.log('5. data.json exists:', !!data.json);
  if (data.json) {
    console.log('6. data.json.choices exists:', !!data.json.choices);
    if (data.json.choices) {
      console.log('7. choices length:', data.json.choices.length);
      if (data.json.choices.length > 0) {
        const firstChoice = data.json.choices[0];
        console.log('8. firstChoice type:', typeof firstChoice);
        console.log('9. firstChoice.message exists:', !!firstChoice.message);
        if (firstChoice.message) {
          const message = firstChoice.message;
          console.log('10. message type:', typeof message);
          console.log('11. message.content exists:', !!message.content);
          if (message.content) {
            const content = message.content;
            console.log('12. content type:', typeof content);
            console.log('13. About to return content');
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
  const queryText = 'This is a test query from the database simulation';
  console.log('1. Got from mock DB:', queryText.substring(0, 50));

  const result = await fetchDeepNested(queryText);
  console.log('14. Got result back:', result);
  console.log('15. Result type:', typeof result);
  console.log('16. Result length:', result.length);

  console.log('Done');
}

main().catch(e => console.error('Error:', e));
