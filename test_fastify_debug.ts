// Debug Fastify - step by step
import Fastify from 'fastify';

console.log("Step 1: Creating app");
const app = Fastify();
console.log("Step 2: App created");

// Define a simple path
const path = '/';
console.log("Step 3: Path defined");

// Try a very simple handler
const handler = async () => {
  return { ok: true };
};
console.log("Step 4: Handler defined");

// This is where it crashes
console.log("Step 5: About to register route");
app.get(path, handler);
console.log("Step 6: Route registered");

console.log("Done");
