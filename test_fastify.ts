// Test Fastify-compatible HTTP framework
// Note: This test file documents the target API - full codegen integration is pending

import Fastify from 'fastify';

const app = Fastify();

// Middleware/hooks
app.addHook('preHandler', async (request, reply) => {
  console.log(`${request.method} ${request.url}`);
});

// Error handler
app.setErrorHandler(async (err, request, reply) => {
  reply.status(500).send({ error: err.message });
});

// Routes - Fastify style
app.get('/', async (request, reply) => {
  return { hello: 'world' };
});

app.get('/users/:id', async (request, reply) => {
  return { id: request.params.id };
});

app.post('/users', async (request, reply) => {
  reply.status(201);
  return { created: true };
});

// Routes - Hono style
app.get('/hono/:name', async (c) => {
  return c.json({ name: c.req.param('name') });
});

app.post('/echo', async (c) => {
  const body = await c.req.json();
  return c.json(body, 201);
});

// Plugin
app.register(async (fastify) => {
  fastify.get('/health', async () => ({ status: 'ok' }));
}, { prefix: '/api' });

app.listen({ port: 3000 }, (err, address) => {
  if (err) {
    console.error(err);
    return;
  }
  console.log(`Server listening on ${address}`);
});
