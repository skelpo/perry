// Test nested JSON array .length

console.log("Starting tests...");

// Test 1: Simple JSON.parse
const simple = JSON.parse('{"items":[1,2,3]}');
console.log("Test 1 - simple.items.length:", simple.items.length);

// Test 2: Nested JSON.parse - mimics POST response structure
const nested = JSON.parse('{"json":{"choices":[{"id":1},{"id":2}]}}');
console.log("Test 2 - nested.json.choices.length:", nested.json.choices.length);

// Test 3: Deeper nesting
const deep = JSON.parse('{"data":{"response":{"items":[1,2,3,4]}}}');
console.log("Test 3 - deep.data.response.items.length:", deep.data.response.items.length);

// Test 4: Array at root of nested object
const root = JSON.parse('{"choices":[{"text":"a"},{"text":"b"}]}');
console.log("Test 4 - root.choices.length:", root.choices.length);

// Test 5: OpenAI-like structure
const openai = JSON.parse('{"id":"test","choices":[{"message":{"content":"hello"}},{"message":{"content":"world"}}]}');
console.log("Test 5 - openai.choices.length:", openai.choices.length);

console.log("All tests passed!");
