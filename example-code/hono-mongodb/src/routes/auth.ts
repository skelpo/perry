// Auth routes
import { Hono } from 'hono';
import { z } from 'zod';
import bcrypt from 'bcryptjs';
import jwt from 'jsonwebtoken';
import { getCollection, User } from '../db';
import { config } from '../config';

export const authRoutes = new Hono();

const registerSchema = z.object({
    username: z.string().min(3).max(30).regex(/^[a-zA-Z0-9_]+$/),
    email: z.string().email(),
    password: z.string().min(8),
    displayName: z.string().min(1).max(100),
});

const loginSchema = z.object({
    email: z.string().email(),
    password: z.string(),
});

// Register
authRoutes.post('/register', async (c) => {
    try {
        const body = await c.req.json();
        const data = registerSchema.parse(body);

        const users = getCollection<User>('users');

        // Check if user exists
        const existing = await users.findOne({
            $or: [{ email: data.email }, { username: data.username }],
        });

        if (existing) {
            return c.json({ error: 'User already exists' }, 409);
        }

        // Hash password
        const passwordHash = await bcrypt.hash(data.password, config.bcrypt.saltRounds);

        // Create user
        const now = new Date();
        const result = await users.insertOne({
            username: data.username,
            email: data.email,
            passwordHash,
            displayName: data.displayName,
            createdAt: now,
            updatedAt: now,
        });

        // Generate token
        const token = jwt.sign(
            { userId: result.insertedId.toString(), email: data.email },
            config.jwt.secret,
            { expiresIn: config.jwt.expiresIn }
        );

        return c.json({
            user: {
                id: result.insertedId.toString(),
                username: data.username,
                email: data.email,
                displayName: data.displayName,
            },
            token,
        }, 201);
    } catch (error) {
        if (error instanceof z.ZodError) {
            return c.json({ error: 'Validation failed', details: error.errors }, 400);
        }
        throw error;
    }
});

// Login
authRoutes.post('/login', async (c) => {
    try {
        const body = await c.req.json();
        const data = loginSchema.parse(body);

        const users = getCollection<User>('users');
        const user = await users.findOne({ email: data.email });

        if (!user) {
            return c.json({ error: 'Invalid credentials' }, 401);
        }

        // Verify password
        const valid = await bcrypt.compare(data.password, user.passwordHash);
        if (!valid) {
            return c.json({ error: 'Invalid credentials' }, 401);
        }

        // Generate token
        const token = jwt.sign(
            { userId: user._id!.toString(), email: user.email },
            config.jwt.secret,
            { expiresIn: config.jwt.expiresIn }
        );

        return c.json({
            user: {
                id: user._id!.toString(),
                username: user.username,
                email: user.email,
                displayName: user.displayName,
            },
            token,
        });
    } catch (error) {
        if (error instanceof z.ZodError) {
            return c.json({ error: 'Validation failed', details: error.errors }, 400);
        }
        throw error;
    }
});

// Get current user
authRoutes.get('/me', async (c) => {
    const authHeader = c.req.header('Authorization');

    if (!authHeader || !authHeader.startsWith('Bearer ')) {
        return c.json({ error: 'Not authenticated' }, 401);
    }

    const token = authHeader.substring(7);

    try {
        const payload = jwt.verify(token, config.jwt.secret) as { userId: string };
        const users = getCollection<User>('users');
        const user = await users.findOne(
            { _id: new (require('mongodb').ObjectId)(payload.userId) },
            { projection: { passwordHash: 0 } }
        );

        if (!user) {
            return c.json({ error: 'User not found' }, 404);
        }

        return c.json(user);
    } catch (error) {
        return c.json({ error: 'Invalid token' }, 401);
    }
});
