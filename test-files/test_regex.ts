// Test file for RegExp support in Perry
// NOTE: regex.test() method not yet supported - test string.replace() only

// Test 1: String.replace() with regex (non-global)
let str = "hello world";
console.log("Test 1: 'hello world'.replace(/world/, 'universe')");
let replaced1 = str.replace(/world/, "universe");
console.log(replaced1);  // should print "hello universe"

// Test 2: String.replace() with global regex
let str2 = "hello hello hello";
console.log("Test 2: 'hello hello hello'.replace(/hello/g, 'hi')");
let replaced2 = str2.replace(/hello/g, "hi");
console.log(replaced2);  // should print "hi hi hi"

// Test 3: Case insensitive replace
let str3 = "Hello World";
console.log("Test 3: Case insensitive replace");
let replaced3 = str3.replace(/hello/i, "HI");
console.log(replaced3);  // should print "HI World"

console.log("All regex tests completed!");
