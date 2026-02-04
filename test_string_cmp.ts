// Test string equality comparison
const a = 'hello';
const b = 'hello';
const c = 'world';

console.log('Testing string === comparison:');
console.log('a:', a);
console.log('b:', b);
console.log('c:', c);

const eq1 = a === b;
console.log('a === b:', eq1);

const eq2 = a === c;
console.log('a === c:', eq2);

const ne1 = a !== c;
console.log('a !== c:', ne1);

// Test switch statement with strings
console.log('\nTesting switch with strings:');
const provider = 'openai';

switch (provider) {
    case 'openai':
        console.log('Matched: openai');
        break;
    case 'anthropic':
        console.log('Matched: anthropic');
        break;
    case 'google':
        console.log('Matched: google');
        break;
    default:
        console.log('Matched: default');
}

// Test with different value
const provider2 = 'anthropic';
switch (provider2) {
    case 'openai':
        console.log('Matched: openai');
        break;
    case 'anthropic':
        console.log('Matched: anthropic');
        break;
    default:
        console.log('Matched: default');
}
