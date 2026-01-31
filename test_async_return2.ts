// Test async function returning values - minimal version

async function fetchData(): Promise<string> {
  console.log('1. In fetchData');

  const response = await fetch('https://httpbin.org/json');
  console.log('2. Got response');

  const data = await response.json();
  console.log('3. Parsed JSON');

  const title = data.slideshow.title;
  console.log('4. Got title, about to return');
  return title;
}

async function main(): Promise<void> {
  console.log('Starting...');
  const result = await fetchData();
  console.log('5. Back from await');
  console.log('6. Result type:', typeof result);
  console.log('Done');
}

main().catch(e => console.error('Error:', e));
