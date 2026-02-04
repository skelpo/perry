// Test Fastify route registration
import Fastify from 'fastify';

console.log("Creating app");
const app = Fastify();
console.log("App created");

console.log("Registering route");
app.get('/', async (request, reply) => {
  return { hello: 'world' };
});
console.log("Route registered");

console.log("Done");
