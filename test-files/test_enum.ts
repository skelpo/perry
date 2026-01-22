// Test basic enums

// Numeric enum with auto-increment
enum Color {
    Red,    // 0
    Green,  // 1
    Blue    // 2
}

// Print numeric enum values
console.log(Color.Red);   // Should print 0
console.log(Color.Green); // Should print 1
console.log(Color.Blue);  // Should print 2

// Numeric enum with explicit values
enum Status {
    Pending = 1,
    Active = 5,
    Done = 10
}

console.log(Status.Pending); // Should print 1
console.log(Status.Active);  // Should print 5
console.log(Status.Done);    // Should print 10

// Use enum in comparison
let myColor = Color.Green;
if (myColor === Color.Green) {
    console.log(100); // Should print 100 (means test passed)
}
