// Job queue routes using Redis lists
import Router from 'koa-router';
import { z } from 'zod';
import { v4 as uuidv4 } from 'uuid';
import { redis } from '../index';

export const queueRoutes = new Router({ prefix: '/api/queue' });

interface Job {
    id: string;
    type: string;
    payload: Record<string, unknown>;
    status: 'pending' | 'processing' | 'completed' | 'failed';
    attempts: number;
    maxAttempts: number;
    createdAt: string;
    startedAt?: string;
    completedAt?: string;
    error?: string;
}

const createJobSchema = z.object({
    type: z.string().min(1),
    payload: z.record(z.unknown()).default({}),
    maxAttempts: z.number().int().positive().default(3),
});

// Queue names
const PENDING_QUEUE = 'queue:pending';
const PROCESSING_QUEUE = 'queue:processing';
const COMPLETED_QUEUE = 'queue:completed';
const FAILED_QUEUE = 'queue:failed';

// Get queue stats
queueRoutes.get('/stats', async (ctx) => {
    const [pending, processing, completed, failed] = await Promise.all([
        redis.llen(PENDING_QUEUE),
        redis.llen(PROCESSING_QUEUE),
        redis.llen(COMPLETED_QUEUE),
        redis.llen(FAILED_QUEUE),
    ]);

    ctx.body = {
        pending,
        processing,
        completed,
        failed,
        total: pending + processing + completed + failed,
    };
});

// Add job to queue
queueRoutes.post('/jobs', async (ctx) => {
    try {
        const data = createJobSchema.parse(ctx.request.body);

        const job: Job = {
            id: uuidv4(),
            type: data.type,
            payload: data.payload,
            status: 'pending',
            attempts: 0,
            maxAttempts: data.maxAttempts,
            createdAt: new Date().toISOString(),
        };

        // Store job data
        await redis.set(`job:${job.id}`, JSON.stringify(job));

        // Add to pending queue
        await redis.rpush(PENDING_QUEUE, job.id);

        ctx.status = 201;
        ctx.body = job;
    } catch (error) {
        if (error instanceof z.ZodError) {
            ctx.status = 400;
            ctx.body = { error: 'Validation failed', details: error.errors };
            return;
        }
        throw error;
    }
});

// Get next job from queue (for workers)
queueRoutes.post('/jobs/next', async (ctx) => {
    // Move job from pending to processing (atomic)
    const jobId = await redis.rpoplpush(PENDING_QUEUE, PROCESSING_QUEUE);

    if (!jobId) {
        ctx.status = 204;
        return;
    }

    const jobData = await redis.get(`job:${jobId}`);
    if (!jobData) {
        ctx.status = 404;
        ctx.body = { error: 'Job data not found' };
        return;
    }

    const job: Job = JSON.parse(jobData);
    job.status = 'processing';
    job.attempts += 1;
    job.startedAt = new Date().toISOString();

    await redis.set(`job:${job.id}`, JSON.stringify(job));

    ctx.body = job;
});

// Complete job
queueRoutes.post('/jobs/:id/complete', async (ctx) => {
    const { id } = ctx.params;

    const jobData = await redis.get(`job:${id}`);
    if (!jobData) {
        ctx.status = 404;
        ctx.body = { error: 'Job not found' };
        return;
    }

    const job: Job = JSON.parse(jobData);
    job.status = 'completed';
    job.completedAt = new Date().toISOString();

    // Update job data
    await redis.set(`job:${id}`, JSON.stringify(job));

    // Move from processing to completed
    await redis.lrem(PROCESSING_QUEUE, 1, id);
    await redis.rpush(COMPLETED_QUEUE, id);

    ctx.body = job;
});

// Fail job
queueRoutes.post('/jobs/:id/fail', async (ctx) => {
    const { id } = ctx.params;
    const { error: errorMessage } = ctx.request.body as { error?: string };

    const jobData = await redis.get(`job:${id}`);
    if (!jobData) {
        ctx.status = 404;
        ctx.body = { error: 'Job not found' };
        return;
    }

    const job: Job = JSON.parse(jobData);
    job.error = errorMessage || 'Unknown error';

    // Check if we should retry
    if (job.attempts < job.maxAttempts) {
        job.status = 'pending';
        await redis.set(`job:${id}`, JSON.stringify(job));

        // Move back to pending for retry
        await redis.lrem(PROCESSING_QUEUE, 1, id);
        await redis.rpush(PENDING_QUEUE, id);

        ctx.body = { ...job, message: 'Job queued for retry' };
    } else {
        job.status = 'failed';
        job.completedAt = new Date().toISOString();
        await redis.set(`job:${id}`, JSON.stringify(job));

        // Move to failed queue
        await redis.lrem(PROCESSING_QUEUE, 1, id);
        await redis.rpush(FAILED_QUEUE, id);

        ctx.body = { ...job, message: 'Job failed permanently' };
    }
});

// Get job by ID
queueRoutes.get('/jobs/:id', async (ctx) => {
    const { id } = ctx.params;

    const jobData = await redis.get(`job:${id}`);
    if (!jobData) {
        ctx.status = 404;
        ctx.body = { error: 'Job not found' };
        return;
    }

    ctx.body = JSON.parse(jobData);
});

// Get jobs from a specific queue
queueRoutes.get('/jobs', async (ctx) => {
    const status = ctx.query.status as string || 'pending';
    const limit = parseInt(ctx.query.limit as string || '10', 10);

    let queueName: string;
    switch (status) {
        case 'processing':
            queueName = PROCESSING_QUEUE;
            break;
        case 'completed':
            queueName = COMPLETED_QUEUE;
            break;
        case 'failed':
            queueName = FAILED_QUEUE;
            break;
        default:
            queueName = PENDING_QUEUE;
    }

    const jobIds = await redis.lrange(queueName, 0, limit - 1);

    if (jobIds.length === 0) {
        ctx.body = [];
        return;
    }

    const pipeline = redis.pipeline();
    for (const id of jobIds) {
        pipeline.get(`job:${id}`);
    }

    const results = await pipeline.exec();
    const jobs: Job[] = [];

    for (const result of results || []) {
        if (result && result[1]) {
            jobs.push(JSON.parse(result[1] as string));
        }
    }

    ctx.body = jobs;
});

// Clear completed jobs
queueRoutes.delete('/jobs/completed', async (ctx) => {
    const jobIds = await redis.lrange(COMPLETED_QUEUE, 0, -1);

    const pipeline = redis.pipeline();
    for (const id of jobIds) {
        pipeline.del(`job:${id}`);
    }
    await pipeline.exec();

    await redis.del(COMPLETED_QUEUE);

    ctx.body = { deleted: jobIds.length };
});
