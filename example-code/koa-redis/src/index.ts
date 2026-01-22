// Koa + Redis Example
import Koa from 'koa';
import Router from 'koa-router';
import bodyParser from 'koa-bodyparser';
import helmet from 'koa-helmet';
import { config } from './config';
import { createRedisClient } from './cache';
import { taskRoutes } from './routes/tasks';
import { queueRoutes } from './routes/queue';
import { leaderboardRoutes } from './routes/leaderboard';

const app = new Koa();
const router = new Router();

// Initialize Redis
export const redis = createRedisClient();

// Middleware
app.use(helmet());
app.use(bodyParser());

// Error handling
app.use(async (ctx, next) => {
    try {
        await next();
    } catch (err) {
        const error = err as Error;
        console.error('Error:', error.message);
        ctx.status = 500;
        ctx.body = { error: 'Internal server error' };
    }
});

// Request logging
app.use(async (ctx, next) => {
    const start = Date.now();
    await next();
    const ms = Date.now() - start;
    console.log(`${ctx.method} ${ctx.url} - ${ctx.status} - ${ms}ms`);
});

// Health check
router.get('/health', async (ctx) => {
    try {
        await redis.ping();
        ctx.body = {
            status: 'healthy',
            redis: 'connected',
            timestamp: new Date().toISOString(),
        };
    } catch (error) {
        ctx.status = 503;
        ctx.body = { status: 'unhealthy', error: 'Redis unavailable' };
    }
});

// Mount routes
app.use(router.routes());
app.use(router.allowedMethods());
app.use(taskRoutes.routes());
app.use(taskRoutes.allowedMethods());
app.use(queueRoutes.routes());
app.use(queueRoutes.allowedMethods());
app.use(leaderboardRoutes.routes());
app.use(leaderboardRoutes.allowedMethods());

// Start server
app.listen(config.port, () => {
    console.log(`Koa server running on port ${config.port}`);
});

// Graceful shutdown
process.on('SIGTERM', () => {
    console.log('Shutting down...');
    redis.disconnect();
    process.exit(0);
});

export { app };
