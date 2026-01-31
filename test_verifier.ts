// Test file to reproduce the verifier error
// Simple function with string and any parameters
function logMessage(message: string, data?: any): void {
    if (data) {
        console.log("With data:", message);
    } else {
        console.log("No data:", message);
    }
}

logMessage("Hello");
logMessage("Test", { value: 123 });
