// Test object JSON serialization in console.log

// Simple object
const obj = { name: 'Alice', age: 30 };
console.log('Simple object:', obj);

// Nested object
const nested = {
    person: { name: 'Bob', age: 25 },
    active: true
};
console.log('Nested object:', nested);

// Object with array
const withArray = {
    items: [1, 2, 3],
    count: 3
};
console.log('Object with array:', withArray);

// Just the object (not as part of spread)
console.log(obj);
