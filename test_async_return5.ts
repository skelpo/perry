// Test async function with if block - with diagnostics

async function fetchData(): Promise<string> {
  console.log('A. In fetchData');
  const response = await fetch('https://httpbin.org/json');
  console.log('B. Got response');
  const data = await response.json();
  console.log('C. Parsed JSON');

  if (data.slideshow) {
    console.log('D. Inside if block');
    const title = data.slideshow.title;
    console.log('E. Got title:', title);
    console.log('F. About to return');
    return title;
  }

  console.log('G. After if block (should not reach)');
  return 'no title';
}

async function main(): Promise<void> {
  console.log('1. Starting...');
  const result = await fetchData();
  console.log('2. Got result:', result);
  console.log('3. Done');
}

main().catch(e => console.error('Error:', e));
