// Redis Cache Client
import Redis from 'ioredis';
import { config } from './config';

export function createRedisClient(): Redis {
    const redis = new Redis({
        host: config.redis.host,
        port: config.redis.port,
        password: config.redis.password,
        db: config.redis.db,
        retryStrategy: (times: number) => {
            const delay = Math.min(times * 50, 2000);
            return delay;
        },
    });

    redis.on('connect', () => {
        console.log('Redis connected');
    });

    redis.on('error', (err: Error) => {
        console.error('Redis error:', err.message);
    });

    return redis;
}

// Cache helper functions
export class CacheService {
    constructor(private redis: Redis) {}

    async get<T>(key: string): Promise<T | null> {
        const data = await this.redis.get(key);
        if (!data) return null;
        return JSON.parse(data) as T;
    }

    async set(key: string, value: unknown, ttlSeconds: number): Promise<void> {
        await this.redis.setex(key, ttlSeconds, JSON.stringify(value));
    }

    async del(key: string): Promise<void> {
        await this.redis.del(key);
    }

    async invalidatePattern(pattern: string): Promise<void> {
        const keys = await this.redis.keys(pattern);
        if (keys.length > 0) {
            await this.redis.del(...keys);
        }
    }
}
