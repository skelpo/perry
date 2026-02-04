// Debug Fastify - with named function handler
import Fastify from 'fastify';

function handler() {
  return true;
}

const app = Fastify();
console.log("App created");

console.log("Registering route");
app.get('/', handler);
console.log("Route registered");

console.log("Done");
