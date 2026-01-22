// Hono + MongoDB Example
import { serve } from '@hono/node-server';
import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { logger } from 'hono/logger';
import { prettyJSON } from 'hono/pretty-json';
import { config } from './config';
import { connectDB, getDB } from './db';
import { authRoutes } from './routes/auth';
import { postRoutes } from './routes/posts';
import { commentRoutes } from './routes/comments';
import { authMiddleware } from './middleware/auth';

const app = new Hono();

// Middleware
app.use('*', logger());
app.use('*', cors());
app.use('*', prettyJSON());

// Health check
app.get('/health', async (c) => {
    try {
        const db = getDB();
        await db.command({ ping: 1 });

        return c.json({
            status: 'healthy',
            database: 'connected',
            timestamp: new Date().toISOString(),
        });
    } catch (error) {
        return c.json({ status: 'unhealthy', error: 'Database unavailable' }, 503);
    }
});

// Public routes
app.route('/api/auth', authRoutes);

// Protected routes
app.use('/api/posts/*', authMiddleware);
app.use('/api/comments/*', authMiddleware);
app.route('/api/posts', postRoutes);
app.route('/api/comments', commentRoutes);

// 404 handler
app.notFound((c) => {
    return c.json({ error: 'Not found' }, 404);
});

// Error handler
app.onError((err, c) => {
    console.error('Error:', err.message);
    return c.json({ error: 'Internal server error' }, 500);
});

// Start server
async function main() {
    await connectDB();

    serve({
        fetch: app.fetch,
        port: config.port,
    });

    console.log(`Hono server running on port ${config.port}`);
}

main().catch(console.error);

export { app };
