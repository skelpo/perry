// Express + PostgreSQL Example
import express, { Request, Response, NextFunction } from 'express';
import { Pool } from 'pg';
import cors from 'cors';
import helmet from 'helmet';
import { config } from './config';
import { userRouter } from './routes/users';
import { productRouter } from './routes/products';

const app = express();

// Middleware
app.use(helmet());
app.use(cors());
app.use(express.json());

// Database connection pool
export const pool = new Pool({
    host: config.db.host,
    port: config.db.port,
    database: config.db.database,
    user: config.db.user,
    password: config.db.password,
    max: 20,
    idleTimeoutMillis: 30000,
});

// Health check
app.get('/health', async (req: Request, res: Response) => {
    try {
        const result = await pool.query('SELECT NOW()');
        res.json({
            status: 'healthy',
            timestamp: result.rows[0].now,
            uptime: process.uptime()
        });
    } catch (error) {
        res.status(503).json({ status: 'unhealthy', error: 'Database connection failed' });
    }
});

// Routes
app.use('/api/users', userRouter);
app.use('/api/products', productRouter);

// Error handling middleware
app.use((err: Error, req: Request, res: Response, next: NextFunction) => {
    console.error('Error:', err.message);
    res.status(500).json({ error: 'Internal server error' });
});

// Start server
const PORT = config.port;
app.listen(PORT, () => {
    console.log(`Express server running on port ${PORT}`);
});

export { app };
