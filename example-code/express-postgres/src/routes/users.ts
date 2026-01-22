// User routes
import { Router, Request, Response } from 'express';
import { z } from 'zod';
import { pool } from '../index';

export const userRouter = Router();

// Validation schemas
const createUserSchema = z.object({
    name: z.string().min(1).max(100),
    email: z.string().email(),
    age: z.number().int().positive().optional(),
});

const updateUserSchema = createUserSchema.partial();

// Get all users
userRouter.get('/', async (req: Request, res: Response) => {
    try {
        const { rows } = await pool.query(
            'SELECT id, name, email, age, created_at FROM users ORDER BY created_at DESC'
        );
        res.json(rows);
    } catch (error) {
        res.status(500).json({ error: 'Failed to fetch users' });
    }
});

// Get user by ID
userRouter.get('/:id', async (req: Request, res: Response) => {
    try {
        const { id } = req.params;
        const { rows } = await pool.query(
            'SELECT id, name, email, age, created_at FROM users WHERE id = $1',
            [id]
        );

        if (rows.length === 0) {
            return res.status(404).json({ error: 'User not found' });
        }

        res.json(rows[0]);
    } catch (error) {
        res.status(500).json({ error: 'Failed to fetch user' });
    }
});

// Create user
userRouter.post('/', async (req: Request, res: Response) => {
    try {
        const data = createUserSchema.parse(req.body);

        const { rows } = await pool.query(
            'INSERT INTO users (name, email, age) VALUES ($1, $2, $3) RETURNING *',
            [data.name, data.email, data.age]
        );

        res.status(201).json(rows[0]);
    } catch (error) {
        if (error instanceof z.ZodError) {
            return res.status(400).json({ error: 'Validation failed', details: error.errors });
        }
        res.status(500).json({ error: 'Failed to create user' });
    }
});

// Update user
userRouter.put('/:id', async (req: Request, res: Response) => {
    try {
        const { id } = req.params;
        const data = updateUserSchema.parse(req.body);

        const fields: string[] = [];
        const values: (string | number)[] = [];
        let paramIndex = 1;

        if (data.name !== undefined) {
            fields.push(`name = $${paramIndex++}`);
            values.push(data.name);
        }
        if (data.email !== undefined) {
            fields.push(`email = $${paramIndex++}`);
            values.push(data.email);
        }
        if (data.age !== undefined) {
            fields.push(`age = $${paramIndex++}`);
            values.push(data.age);
        }

        if (fields.length === 0) {
            return res.status(400).json({ error: 'No fields to update' });
        }

        values.push(parseInt(id, 10));

        const { rows } = await pool.query(
            `UPDATE users SET ${fields.join(', ')} WHERE id = $${paramIndex} RETURNING *`,
            values
        );

        if (rows.length === 0) {
            return res.status(404).json({ error: 'User not found' });
        }

        res.json(rows[0]);
    } catch (error) {
        if (error instanceof z.ZodError) {
            return res.status(400).json({ error: 'Validation failed', details: error.errors });
        }
        res.status(500).json({ error: 'Failed to update user' });
    }
});

// Delete user
userRouter.delete('/:id', async (req: Request, res: Response) => {
    try {
        const { id } = req.params;
        const { rowCount } = await pool.query('DELETE FROM users WHERE id = $1', [id]);

        if (rowCount === 0) {
            return res.status(404).json({ error: 'User not found' });
        }

        res.status(204).send();
    } catch (error) {
        res.status(500).json({ error: 'Failed to delete user' });
    }
});
