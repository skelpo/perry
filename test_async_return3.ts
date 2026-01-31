// Test async function returning values - test printing result

async function fetchData(): Promise<string> {
  const response = await fetch('https://httpbin.org/json');
  const data = await response.json();
  const title = data.slideshow.title;
  return title;
}

async function main(): Promise<void> {
  console.log('Starting...');
  const result = await fetchData();
  console.log('Got result type:', typeof result);
  console.log('Got result:', result);
  console.log('Done');
}

main().catch(e => console.error('Error:', e));
