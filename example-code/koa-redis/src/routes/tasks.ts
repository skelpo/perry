// Task routes with Redis storage
import Router from 'koa-router';
import { z } from 'zod';
import { v4 as uuidv4 } from 'uuid';
import { redis } from '../index';
import { config } from '../config';

export const taskRoutes = new Router({ prefix: '/api/tasks' });

interface Task {
    id: string;
    title: string;
    description: string;
    status: 'pending' | 'in_progress' | 'completed';
    priority: number;
    assignee?: string;
    createdAt: string;
    updatedAt: string;
}

const createTaskSchema = z.object({
    title: z.string().min(1).max(200),
    description: z.string().default(''),
    priority: z.number().int().min(1).max(5).default(3),
    assignee: z.string().optional(),
});

const updateTaskSchema = z.object({
    title: z.string().min(1).max(200).optional(),
    description: z.string().optional(),
    status: z.enum(['pending', 'in_progress', 'completed']).optional(),
    priority: z.number().int().min(1).max(5).optional(),
    assignee: z.string().optional(),
});

// Get all tasks
taskRoutes.get('/', async (ctx) => {
    const keys = await redis.keys('task:*');

    if (keys.length === 0) {
        ctx.body = [];
        return;
    }

    const pipeline = redis.pipeline();
    for (const key of keys) {
        pipeline.get(key);
    }

    const results = await pipeline.exec();
    const tasks: Task[] = [];

    for (const result of results || []) {
        if (result && result[1]) {
            tasks.push(JSON.parse(result[1] as string));
        }
    }

    // Sort by priority (high to low) then by creation date
    tasks.sort((a, b) => {
        if (a.priority !== b.priority) return b.priority - a.priority;
        return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
    });

    ctx.body = tasks;
});

// Get task by ID
taskRoutes.get('/:id', async (ctx) => {
    const { id } = ctx.params;
    const data = await redis.get(`task:${id}`);

    if (!data) {
        ctx.status = 404;
        ctx.body = { error: 'Task not found' };
        return;
    }

    ctx.body = JSON.parse(data);
});

// Create task
taskRoutes.post('/', async (ctx) => {
    try {
        const data = createTaskSchema.parse(ctx.request.body);

        const task: Task = {
            id: uuidv4(),
            title: data.title,
            description: data.description,
            status: 'pending',
            priority: data.priority,
            assignee: data.assignee,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        };

        await redis.setex(`task:${task.id}`, config.cache.taskTTL * 1000, JSON.stringify(task));

        // Add to priority index
        await redis.zadd('tasks:by_priority', task.priority, task.id);

        ctx.status = 201;
        ctx.body = task;
    } catch (error) {
        if (error instanceof z.ZodError) {
            ctx.status = 400;
            ctx.body = { error: 'Validation failed', details: error.errors };
            return;
        }
        throw error;
    }
});

// Update task
taskRoutes.put('/:id', async (ctx) => {
    try {
        const { id } = ctx.params;
        const updates = updateTaskSchema.parse(ctx.request.body);

        const existing = await redis.get(`task:${id}`);
        if (!existing) {
            ctx.status = 404;
            ctx.body = { error: 'Task not found' };
            return;
        }

        const task: Task = {
            ...JSON.parse(existing),
            ...updates,
            updatedAt: new Date().toISOString(),
        };

        await redis.setex(`task:${id}`, config.cache.taskTTL * 1000, JSON.stringify(task));

        // Update priority index if priority changed
        if (updates.priority !== undefined) {
            await redis.zadd('tasks:by_priority', task.priority, task.id);
        }

        ctx.body = task;
    } catch (error) {
        if (error instanceof z.ZodError) {
            ctx.status = 400;
            ctx.body = { error: 'Validation failed', details: error.errors };
            return;
        }
        throw error;
    }
});

// Delete task
taskRoutes.delete('/:id', async (ctx) => {
    const { id } = ctx.params;

    const deleted = await redis.del(`task:${id}`);
    await redis.zrem('tasks:by_priority', id);

    if (deleted === 0) {
        ctx.status = 404;
        ctx.body = { error: 'Task not found' };
        return;
    }

    ctx.status = 204;
});

// Get tasks by priority range
taskRoutes.get('/priority/:min/:max', async (ctx) => {
    const min = parseInt(ctx.params.min, 10);
    const max = parseInt(ctx.params.max, 10);

    const taskIds = await redis.zrangebyscore('tasks:by_priority', min, max);

    if (taskIds.length === 0) {
        ctx.body = [];
        return;
    }

    const pipeline = redis.pipeline();
    for (const id of taskIds) {
        pipeline.get(`task:${id}`);
    }

    const results = await pipeline.exec();
    const tasks: Task[] = [];

    for (const result of results || []) {
        if (result && result[1]) {
            tasks.push(JSON.parse(result[1] as string));
        }
    }

    ctx.body = tasks;
});
