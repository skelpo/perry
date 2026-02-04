// Simple Fastify test - just verify the basic API works
import Fastify from 'fastify';

const app = Fastify();

console.log("Created Fastify app");

// Register a simple route
app.get('/', async (request, reply) => {
  console.log("Handler called for /");
  return { hello: 'world' };
});

console.log("Registered GET / route");

app.get('/test', async (request, reply) => {
  console.log("Handler called for /test");
  return { status: 'ok' };
});

console.log("Registered GET /test route");

// Just test that we can call the methods without errors
console.log("Fastify app setup complete");
