// Order routes with MySQL transactions
import { FastifyInstance, FastifyRequest, FastifyReply } from 'fastify';
import { z } from 'zod';
import { db, redis } from '../index';
import { CacheService } from '../cache';
import { config } from '../config';
import { PoolConnection } from 'mysql2/promise';

const cache = new CacheService(redis);

const createOrderSchema = z.object({
    userId: z.number().int().positive(),
    items: z.array(z.object({
        productId: z.number().int().positive(),
        quantity: z.number().int().positive(),
        price: z.number().positive(),
    })).min(1),
    shippingAddress: z.string().min(10),
});

interface OrderItem {
    productId: number;
    quantity: number;
    price: number;
}

export async function orderRoutes(app: FastifyInstance) {
    // Get order by ID
    app.get<{ Params: { id: string } }>('/:id', async (request, reply) => {
        const { id } = request.params;
        const cacheKey = `order:${id}`;

        // Try cache
        const cached = await cache.get<Record<string, unknown>>(cacheKey);
        if (cached) {
            return cached;
        }

        // Get order with items
        const [orderRows] = await db.query('SELECT * FROM orders WHERE id = ?', [id]);
        const orders = orderRows as Record<string, unknown>[];

        if (orders.length === 0) {
            reply.status(404);
            return { error: 'Order not found' };
        }

        const [itemRows] = await db.query(
            'SELECT * FROM order_items WHERE order_id = ?',
            [id]
        );

        const order = {
            ...orders[0],
            items: itemRows,
        };

        await cache.set(cacheKey, order, config.cache.orderTTL);

        return order;
    });

    // Get orders for user
    app.get('/user/:userId', async (request, reply) => {
        const { userId } = request.params as { userId: string };

        const [rows] = await db.query(
            `SELECT o.*, COUNT(oi.id) as item_count, SUM(oi.quantity * oi.price) as total
             FROM orders o
             LEFT JOIN order_items oi ON o.id = oi.order_id
             WHERE o.user_id = ?
             GROUP BY o.id
             ORDER BY o.created_at DESC`,
            [userId]
        );

        return rows;
    });

    // Create order with transaction
    app.post('/', async (request, reply) => {
        const data = createOrderSchema.parse(request.body);

        const connection = await db.getConnection();

        try {
            await connection.beginTransaction();

            // Calculate total
            const total = data.items.reduce(
                (sum: number, item: OrderItem) => sum + item.quantity * item.price,
                0
            );

            // Create order
            const [orderResult] = await connection.query(
                `INSERT INTO orders (user_id, status, total, shipping_address)
                 VALUES (?, 'pending', ?, ?)`,
                [data.userId, total, data.shippingAddress]
            );

            const orderId = (orderResult as { insertId: number }).insertId;

            // Insert order items
            for (const item of data.items) {
                await connection.query(
                    `INSERT INTO order_items (order_id, product_id, quantity, price)
                     VALUES (?, ?, ?, ?)`,
                    [orderId, item.productId, item.quantity, item.price]
                );

                // Update product stock
                await connection.query(
                    'UPDATE products SET stock = stock - ? WHERE id = ? AND stock >= ?',
                    [item.quantity, item.productId, item.quantity]
                );
            }

            await connection.commit();

            reply.status(201);
            return {
                id: orderId,
                status: 'pending',
                total,
                itemCount: data.items.length,
            };
        } catch (error) {
            await connection.rollback();
            throw error;
        } finally {
            connection.release();
        }
    });

    // Update order status
    app.patch<{ Params: { id: string } }>('/:id/status', async (request, reply) => {
        const { id } = request.params;
        const { status } = request.body as { status: string };

        const validStatuses = ['pending', 'processing', 'shipped', 'delivered', 'cancelled'];
        if (!validStatuses.includes(status)) {
            reply.status(400);
            return { error: 'Invalid status' };
        }

        const [result] = await db.query(
            'UPDATE orders SET status = ? WHERE id = ?',
            [status, id]
        );

        const updateResult = result as { affectedRows: number };

        if (updateResult.affectedRows === 0) {
            reply.status(404);
            return { error: 'Order not found' };
        }

        // Invalidate cache
        await cache.del(`order:${id}`);

        return { id, status };
    });

    // Cancel order
    app.post<{ Params: { id: string } }>('/:id/cancel', async (request, reply) => {
        const { id } = request.params;

        const connection = await db.getConnection();

        try {
            await connection.beginTransaction();

            // Check if order can be cancelled
            const [orderRows] = await connection.query(
                'SELECT * FROM orders WHERE id = ? AND status IN ("pending", "processing")',
                [id]
            );

            const orders = orderRows as Record<string, unknown>[];
            if (orders.length === 0) {
                reply.status(400);
                return { error: 'Order cannot be cancelled' };
            }

            // Restore product stock
            const [items] = await connection.query(
                'SELECT product_id, quantity FROM order_items WHERE order_id = ?',
                [id]
            );

            for (const item of items as { product_id: number; quantity: number }[]) {
                await connection.query(
                    'UPDATE products SET stock = stock + ? WHERE id = ?',
                    [item.quantity, item.product_id]
                );
            }

            // Update order status
            await connection.query(
                'UPDATE orders SET status = "cancelled" WHERE id = ?',
                [id]
            );

            await connection.commit();

            // Invalidate cache
            await cache.del(`order:${id}`);

            return { id, status: 'cancelled' };
        } catch (error) {
            await connection.rollback();
            throw error;
        } finally {
            connection.release();
        }
    });
}
