// Test local array operations to ensure they still work
const names = ['ethereum', 'base', 'bnb'];
console.log('Joined:', names.join(', '));
console.log('Length:', names.length);
console.log('First:', names[0]);
console.log('Includes base:', names.includes('base'));
console.log('Index of bnb:', names.indexOf('bnb'));

// Test map
const upper = names.map(n => n.toUpperCase());
console.log('Upper:', upper.join(', '));

// Test filter
const short = names.filter(n => n.length < 5);
console.log('Short:', short.join(', '));
