// Fastify + Redis + MySQL Example
import Fastify, { FastifyInstance } from 'fastify';
import cors from '@fastify/cors';
import helmet from '@fastify/helmet';
import rateLimit from '@fastify/rate-limit';
import { config } from './config';
import { createPool } from './db';
import { createRedisClient } from './cache';
import { sessionRoutes } from './routes/sessions';
import { orderRoutes } from './routes/orders';

const app: FastifyInstance = Fastify({
    logger: {
        level: config.logLevel,
    },
});

// Initialize connections
export const db = createPool();
export const redis = createRedisClient();

async function main() {
    // Register plugins
    await app.register(cors, { origin: true });
    await app.register(helmet);
    await app.register(rateLimit, {
        max: 100,
        timeWindow: '1 minute',
    });

    // Health check
    app.get('/health', async (request, reply) => {
        try {
            // Check MySQL
            const [rows] = await db.query('SELECT 1');

            // Check Redis
            await redis.ping();

            return {
                status: 'healthy',
                mysql: 'connected',
                redis: 'connected',
                timestamp: new Date().toISOString(),
            };
        } catch (error) {
            reply.status(503);
            return { status: 'unhealthy', error: 'Service unavailable' };
        }
    });

    // Register routes
    await app.register(sessionRoutes, { prefix: '/api/sessions' });
    await app.register(orderRoutes, { prefix: '/api/orders' });

    // Start server
    try {
        await app.listen({ port: config.port, host: '0.0.0.0' });
        console.log(`Fastify server running on port ${config.port}`);
    } catch (err) {
        app.log.error(err);
        process.exit(1);
    }
}

// Graceful shutdown
process.on('SIGTERM', async () => {
    console.log('Shutting down...');
    await app.close();
    await db.end();
    redis.disconnect();
    process.exit(0);
});

main();

export { app };
