// Simple split test
const domain = 'calendly.com';
console.log('1. domain:', domain);

const parts = domain.split('.');
console.log('2. split done');
console.log('3. parts length:', parts.length);

// This might be where it crashes - accessing array element
const first = parts[0];
console.log('4. first:', first);

console.log('5. done');
