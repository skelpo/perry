// Leaderboard routes using Redis sorted sets
import Router from 'koa-router';
import { z } from 'zod';
import { redis } from '../index';

export const leaderboardRoutes = new Router({ prefix: '/api/leaderboard' });

const LEADERBOARD_KEY = 'leaderboard:global';

const scoreSchema = z.object({
    userId: z.string().min(1),
    username: z.string().min(1),
    score: z.number().int(),
});

interface LeaderboardEntry {
    rank: number;
    userId: string;
    username: string;
    score: number;
}

// Get top players
leaderboardRoutes.get('/top/:count?', async (ctx) => {
    const count = parseInt(ctx.params.count || '10', 10);

    // Get top scores (highest first)
    const results = await redis.zrevrange(LEADERBOARD_KEY, 0, count - 1, 'WITHSCORES');

    const entries: LeaderboardEntry[] = [];
    for (let i = 0; i < results.length; i += 2) {
        const userId = results[i];
        const score = parseInt(results[i + 1], 10);

        // Get username
        const username = await redis.hget(`user:${userId}`, 'username') || userId;

        entries.push({
            rank: Math.floor(i / 2) + 1,
            userId,
            username,
            score,
        });
    }

    ctx.body = entries;
});

// Get player rank and score
leaderboardRoutes.get('/player/:userId', async (ctx) => {
    const { userId } = ctx.params;

    const [rank, score, username] = await Promise.all([
        redis.zrevrank(LEADERBOARD_KEY, userId),
        redis.zscore(LEADERBOARD_KEY, userId),
        redis.hget(`user:${userId}`, 'username'),
    ]);

    if (rank === null || score === null) {
        ctx.status = 404;
        ctx.body = { error: 'Player not found on leaderboard' };
        return;
    }

    ctx.body = {
        rank: rank + 1, // 0-indexed to 1-indexed
        userId,
        username: username || userId,
        score: parseInt(score, 10),
    };
});

// Submit or update score
leaderboardRoutes.post('/score', async (ctx) => {
    try {
        const data = scoreSchema.parse(ctx.request.body);

        // Store username
        await redis.hset(`user:${data.userId}`, 'username', data.username);

        // Get current score
        const currentScore = await redis.zscore(LEADERBOARD_KEY, data.userId);

        // Only update if new score is higher (or no previous score)
        if (currentScore === null || data.score > parseInt(currentScore, 10)) {
            await redis.zadd(LEADERBOARD_KEY, data.score, data.userId);

            const rank = await redis.zrevrank(LEADERBOARD_KEY, data.userId);

            ctx.body = {
                userId: data.userId,
                username: data.username,
                score: data.score,
                rank: rank !== null ? rank + 1 : null,
                isNewHighScore: true,
            };
        } else {
            const rank = await redis.zrevrank(LEADERBOARD_KEY, data.userId);

            ctx.body = {
                userId: data.userId,
                username: data.username,
                score: parseInt(currentScore, 10),
                submittedScore: data.score,
                rank: rank !== null ? rank + 1 : null,
                isNewHighScore: false,
            };
        }
    } catch (error) {
        if (error instanceof z.ZodError) {
            ctx.status = 400;
            ctx.body = { error: 'Validation failed', details: error.errors };
            return;
        }
        throw error;
    }
});

// Increment score
leaderboardRoutes.post('/score/:userId/increment', async (ctx) => {
    const { userId } = ctx.params;
    const { amount = 1 } = ctx.request.body as { amount?: number };

    const newScore = await redis.zincrby(LEADERBOARD_KEY, amount, userId);
    const rank = await redis.zrevrank(LEADERBOARD_KEY, userId);
    const username = await redis.hget(`user:${userId}`, 'username');

    ctx.body = {
        userId,
        username: username || userId,
        score: parseInt(newScore, 10),
        rank: rank !== null ? rank + 1 : null,
    };
});

// Get players around a specific player
leaderboardRoutes.get('/player/:userId/neighbors/:count?', async (ctx) => {
    const { userId } = ctx.params;
    const count = parseInt(ctx.params.count || '2', 10);

    const rank = await redis.zrevrank(LEADERBOARD_KEY, userId);

    if (rank === null) {
        ctx.status = 404;
        ctx.body = { error: 'Player not found on leaderboard' };
        return;
    }

    const start = Math.max(0, rank - count);
    const end = rank + count;

    const results = await redis.zrevrange(LEADERBOARD_KEY, start, end, 'WITHSCORES');

    const entries: LeaderboardEntry[] = [];
    for (let i = 0; i < results.length; i += 2) {
        const id = results[i];
        const score = parseInt(results[i + 1], 10);
        const username = await redis.hget(`user:${id}`, 'username') || id;

        entries.push({
            rank: start + Math.floor(i / 2) + 1,
            userId: id,
            username,
            score,
        });
    }

    ctx.body = {
        targetPlayer: userId,
        neighbors: entries,
    };
});

// Get leaderboard stats
leaderboardRoutes.get('/stats', async (ctx) => {
    const [count, topScore, bottomScore] = await Promise.all([
        redis.zcard(LEADERBOARD_KEY),
        redis.zrevrange(LEADERBOARD_KEY, 0, 0, 'WITHSCORES'),
        redis.zrange(LEADERBOARD_KEY, 0, 0, 'WITHSCORES'),
    ]);

    ctx.body = {
        totalPlayers: count,
        highestScore: topScore.length >= 2 ? parseInt(topScore[1], 10) : null,
        lowestScore: bottomScore.length >= 2 ? parseInt(bottomScore[1], 10) : null,
    };
});

// Remove player from leaderboard
leaderboardRoutes.delete('/player/:userId', async (ctx) => {
    const { userId } = ctx.params;

    const removed = await redis.zrem(LEADERBOARD_KEY, userId);
    await redis.del(`user:${userId}`);

    if (removed === 0) {
        ctx.status = 404;
        ctx.body = { error: 'Player not found' };
        return;
    }

    ctx.status = 204;
});

// Reset leaderboard
leaderboardRoutes.delete('/', async (ctx) => {
    await redis.del(LEADERBOARD_KEY);
    ctx.body = { message: 'Leaderboard reset' };
});
