// Test string.split()
const domain = 'calendly.com';
console.log('domain:', domain);

const parts = domain.split('.');
console.log('parts:', parts);
console.log('parts[0]:', parts[0]);
console.log('parts[1]:', parts[1]);

// Test with different strings
const path = '/users/123/profile';
const segments = path.split('/');
console.log('segments:', segments);

// Test indexOf
const text = 'hello world';
const idx = text.indexOf('world');
console.log('indexOf world:', idx);

// Test includes
const hasWorld = text.includes('world');
console.log('includes world:', hasWorld);

// Test startsWith/endsWith
const startsH = text.startsWith('hello');
const endsD = text.endsWith('world');
console.log('startsWith hello:', startsH);
console.log('endsWith world:', endsD);
