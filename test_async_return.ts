// Test async function returning values

async function fetchData(): Promise<string> {
  console.log('1. In fetchData');

  const response = await fetch('https://httpbin.org/json');
  console.log('2. Got response');

  const data = await response.json();
  console.log('3. Parsed JSON');
  console.log('4. data.slideshow exists:', !!data.slideshow);

  if (data.slideshow) {
    console.log('5. slideshow.title:', data.slideshow.title);
    const title = data.slideshow.title;
    console.log('6. About to return:', title);
    return title;
  }

  return 'no title';
}

async function main(): Promise<void> {
  console.log('Starting...');
  const result = await fetchData();
  console.log('7. Got result:', result);
  console.log('Done');
}

main().catch(e => console.error('Error:', e));
