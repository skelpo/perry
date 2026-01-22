// Test decorator support for method decorators
// The @log decorator logs "Calling <method_name>" before method execution
//
// Note: Perry implements @log as a compile-time transformation
// The decorator is a built-in that prints method entry before execution

class Calculator {
    @log
    sum(a: number, b: number): number {
        return a + b;
    }

    @log
    multiply(a: number, b: number): number {
        return a * b;
    }

    @log
    divide(a: number, b: number): number {
        return a / b;
    }

    // Method without decorator for comparison
    subtract(a: number, b: number): number {
        return a - b;
    }
}

// Test the decorated methods
const calc = new Calculator();

// Test decorated method: should print "Calling sum" then the result
console.log("Testing sum(2, 3):");
const result1 = calc.sum(2, 3);
console.log(result1);

// Test decorated method: should print "Calling multiply" then the result
console.log("Testing multiply(4, 5):");
const result2 = calc.multiply(4, 5);
console.log(result2);

// Test decorated method: should print "Calling divide" then the result
console.log("Testing divide(10, 2):");
const result3 = calc.divide(10, 2);
console.log(result3);

// Test method without decorator: no "Calling" message
console.log("Testing subtract(10, 4):");
const result4 = calc.subtract(10, 4);
console.log(result4);
