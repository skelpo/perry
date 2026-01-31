// Test async function with if block

async function fetchData(): Promise<string> {
  const response = await fetch('https://httpbin.org/json');
  const data = await response.json();

  if (data.slideshow) {
    const title = data.slideshow.title;
    return title;
  }

  return 'no title';
}

async function main(): Promise<void> {
  console.log('Starting...');
  const result = await fetchData();
  console.log('Got result:', result);
  console.log('Done');
}

main().catch(e => console.error('Error:', e));
