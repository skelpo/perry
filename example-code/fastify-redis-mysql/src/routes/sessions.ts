// Session routes with Redis caching
import { FastifyInstance, FastifyRequest, FastifyReply } from 'fastify';
import { z } from 'zod';
import { db, redis } from '../index';
import { CacheService } from '../cache';
import { config } from '../config';

const cache = new CacheService(redis);

const createSessionSchema = z.object({
    userId: z.number().int().positive(),
    deviceInfo: z.string().optional(),
    ipAddress: z.string().ip().optional(),
});

export async function sessionRoutes(app: FastifyInstance) {
    // Get session by ID (cached)
    app.get<{ Params: { id: string } }>('/:id', async (request, reply) => {
        const { id } = request.params;
        const cacheKey = `session:${id}`;

        // Try cache first
        const cached = await cache.get<Record<string, unknown>>(cacheKey);
        if (cached) {
            return { ...cached, fromCache: true };
        }

        // Query database
        const [rows] = await db.query(
            'SELECT * FROM sessions WHERE id = ? AND expires_at > NOW()',
            [id]
        );

        const sessions = rows as Record<string, unknown>[];
        if (sessions.length === 0) {
            reply.status(404);
            return { error: 'Session not found or expired' };
        }

        const session = sessions[0];

        // Cache the result
        await cache.set(cacheKey, session, config.cache.sessionTTL);

        return session;
    });

    // Create new session
    app.post('/', async (request, reply) => {
        const data = createSessionSchema.parse(request.body);

        const sessionToken = generateToken();
        const expiresAt = new Date(Date.now() + 24 * 60 * 60 * 1000); // 24 hours

        const [result] = await db.query(
            `INSERT INTO sessions (user_id, token, device_info, ip_address, expires_at)
             VALUES (?, ?, ?, ?, ?)`,
            [data.userId, sessionToken, data.deviceInfo, data.ipAddress, expiresAt]
        );

        const insertResult = result as { insertId: number };

        reply.status(201);
        return {
            id: insertResult.insertId,
            token: sessionToken,
            expiresAt: expiresAt.toISOString(),
        };
    });

    // Refresh session
    app.post<{ Params: { id: string } }>('/:id/refresh', async (request, reply) => {
        const { id } = request.params;

        const newExpiresAt = new Date(Date.now() + 24 * 60 * 60 * 1000);

        const [result] = await db.query(
            'UPDATE sessions SET expires_at = ? WHERE id = ? AND expires_at > NOW()',
            [newExpiresAt, id]
        );

        const updateResult = result as { affectedRows: number };

        if (updateResult.affectedRows === 0) {
            reply.status(404);
            return { error: 'Session not found or expired' };
        }

        // Invalidate cache
        await cache.del(`session:${id}`);

        return { expiresAt: newExpiresAt.toISOString() };
    });

    // Delete session (logout)
    app.delete<{ Params: { id: string } }>('/:id', async (request, reply) => {
        const { id } = request.params;

        await db.query('DELETE FROM sessions WHERE id = ?', [id]);

        // Invalidate cache
        await cache.del(`session:${id}`);

        reply.status(204);
    });

    // Delete all sessions for user
    app.delete('/user/:userId', async (request, reply) => {
        const { userId } = request.params as { userId: string };

        await db.query('DELETE FROM sessions WHERE user_id = ?', [userId]);

        // Invalidate all session caches for user
        await cache.invalidatePattern('session:*');

        reply.status(204);
    });
}

function generateToken(): string {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    let token = '';
    for (let i = 0; i < 64; i++) {
        token += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return token;
}
