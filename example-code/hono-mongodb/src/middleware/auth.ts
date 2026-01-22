// Authentication middleware
import { Context, Next } from 'hono';
import jwt from 'jsonwebtoken';
import { ObjectId } from 'mongodb';
import { config } from '../config';
import { getCollection, User } from '../db';

interface JWTPayload {
    userId: string;
    email: string;
}

export async function authMiddleware(c: Context, next: Next) {
    const authHeader = c.req.header('Authorization');

    if (!authHeader || !authHeader.startsWith('Bearer ')) {
        return c.json({ error: 'No token provided' }, 401);
    }

    const token = authHeader.substring(7);

    try {
        const payload = jwt.verify(token, config.jwt.secret) as JWTPayload;

        // Get user from database
        const users = getCollection<User>('users');
        const user = await users.findOne({ _id: new ObjectId(payload.userId) });

        if (!user) {
            return c.json({ error: 'User not found' }, 401);
        }

        // Attach user to context
        c.set('userId', payload.userId);
        c.set('user', user);

        await next();
    } catch (error) {
        return c.json({ error: 'Invalid token' }, 401);
    }
}
