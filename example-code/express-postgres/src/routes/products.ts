// Product routes
import { Router, Request, Response } from 'express';
import { z } from 'zod';
import { pool } from '../index';

export const productRouter = Router();

// Validation schemas
const createProductSchema = z.object({
    name: z.string().min(1).max(200),
    description: z.string().optional(),
    price: z.number().positive(),
    stock: z.number().int().nonnegative().default(0),
    category: z.string().optional(),
});

// Get all products with pagination
productRouter.get('/', async (req: Request, res: Response) => {
    try {
        const page = parseInt(req.query.page as string) || 1;
        const limit = parseInt(req.query.limit as string) || 10;
        const offset = (page - 1) * limit;

        const { rows } = await pool.query(
            'SELECT * FROM products ORDER BY created_at DESC LIMIT $1 OFFSET $2',
            [limit, offset]
        );

        const countResult = await pool.query('SELECT COUNT(*) FROM products');
        const total = parseInt(countResult.rows[0].count, 10);

        res.json({
            data: rows,
            pagination: {
                page,
                limit,
                total,
                pages: Math.ceil(total / limit),
            },
        });
    } catch (error) {
        res.status(500).json({ error: 'Failed to fetch products' });
    }
});

// Search products
productRouter.get('/search', async (req: Request, res: Response) => {
    try {
        const query = req.query.q as string;

        if (!query) {
            return res.status(400).json({ error: 'Search query required' });
        }

        const { rows } = await pool.query(
            `SELECT * FROM products
             WHERE name ILIKE $1 OR description ILIKE $1
             ORDER BY created_at DESC LIMIT 50`,
            [`%${query}%`]
        );

        res.json(rows);
    } catch (error) {
        res.status(500).json({ error: 'Search failed' });
    }
});

// Get product by ID
productRouter.get('/:id', async (req: Request, res: Response) => {
    try {
        const { id } = req.params;
        const { rows } = await pool.query('SELECT * FROM products WHERE id = $1', [id]);

        if (rows.length === 0) {
            return res.status(404).json({ error: 'Product not found' });
        }

        res.json(rows[0]);
    } catch (error) {
        res.status(500).json({ error: 'Failed to fetch product' });
    }
});

// Create product
productRouter.post('/', async (req: Request, res: Response) => {
    try {
        const data = createProductSchema.parse(req.body);

        const { rows } = await pool.query(
            `INSERT INTO products (name, description, price, stock, category)
             VALUES ($1, $2, $3, $4, $5) RETURNING *`,
            [data.name, data.description, data.price, data.stock, data.category]
        );

        res.status(201).json(rows[0]);
    } catch (error) {
        if (error instanceof z.ZodError) {
            return res.status(400).json({ error: 'Validation failed', details: error.errors });
        }
        res.status(500).json({ error: 'Failed to create product' });
    }
});

// Update stock
productRouter.patch('/:id/stock', async (req: Request, res: Response) => {
    try {
        const { id } = req.params;
        const { quantity } = req.body;

        if (typeof quantity !== 'number') {
            return res.status(400).json({ error: 'Quantity must be a number' });
        }

        const { rows } = await pool.query(
            'UPDATE products SET stock = stock + $1 WHERE id = $2 RETURNING *',
            [quantity, id]
        );

        if (rows.length === 0) {
            return res.status(404).json({ error: 'Product not found' });
        }

        res.json(rows[0]);
    } catch (error) {
        res.status(500).json({ error: 'Failed to update stock' });
    }
});

// Delete product
productRouter.delete('/:id', async (req: Request, res: Response) => {
    try {
        const { id } = req.params;
        const { rowCount } = await pool.query('DELETE FROM products WHERE id = $1', [id]);

        if (rowCount === 0) {
            return res.status(404).json({ error: 'Product not found' });
        }

        res.status(204).send();
    } catch (error) {
        res.status(500).json({ error: 'Failed to delete product' });
    }
});
