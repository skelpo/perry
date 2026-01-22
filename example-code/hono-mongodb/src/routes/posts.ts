// Post routes
import { Hono } from 'hono';
import { z } from 'zod';
import { ObjectId } from 'mongodb';
import { getCollection, Post, User } from '../db';

export const postRoutes = new Hono();

const createPostSchema = z.object({
    title: z.string().min(1).max(200),
    content: z.string().min(1),
    tags: z.array(z.string()).default([]),
    published: z.boolean().default(false),
});

const updatePostSchema = createPostSchema.partial();

// Get all posts (with pagination)
postRoutes.get('/', async (c) => {
    const page = parseInt(c.req.query('page') || '1');
    const limit = parseInt(c.req.query('limit') || '10');
    const tag = c.req.query('tag');
    const search = c.req.query('q');

    const posts = getCollection<Post>('posts');

    const filter: Record<string, unknown> = { published: true };

    if (tag) {
        filter.tags = tag;
    }

    if (search) {
        filter.$text = { $search: search };
    }

    const [results, total] = await Promise.all([
        posts
            .find(filter)
            .sort({ createdAt: -1 })
            .skip((page - 1) * limit)
            .limit(limit)
            .toArray(),
        posts.countDocuments(filter),
    ]);

    // Get author info
    const authorIds = [...new Set(results.map(p => p.authorId.toString()))];
    const users = getCollection<User>('users');
    const authors = await users
        .find({ _id: { $in: authorIds.map(id => new ObjectId(id)) } })
        .project({ passwordHash: 0 })
        .toArray();

    const authorMap = new Map(authors.map(a => [a._id!.toString(), a]));

    const postsWithAuthors = results.map(post => ({
        ...post,
        author: authorMap.get(post.authorId.toString()),
    }));

    return c.json({
        data: postsWithAuthors,
        pagination: {
            page,
            limit,
            total,
            pages: Math.ceil(total / limit),
        },
    });
});

// Get post by ID
postRoutes.get('/:id', async (c) => {
    const { id } = c.req.param();

    if (!ObjectId.isValid(id)) {
        return c.json({ error: 'Invalid post ID' }, 400);
    }

    const posts = getCollection<Post>('posts');
    const post = await posts.findOne({ _id: new ObjectId(id) });

    if (!post) {
        return c.json({ error: 'Post not found' }, 404);
    }

    // Increment view count
    await posts.updateOne(
        { _id: new ObjectId(id) },
        { $inc: { viewCount: 1 } }
    );

    // Get author
    const users = getCollection<User>('users');
    const author = await users.findOne(
        { _id: post.authorId },
        { projection: { passwordHash: 0 } }
    );

    return c.json({ ...post, author });
});

// Create post
postRoutes.post('/', async (c) => {
    try {
        const userId = c.get('userId');
        const body = await c.req.json();
        const data = createPostSchema.parse(body);

        const posts = getCollection<Post>('posts');
        const now = new Date();

        const result = await posts.insertOne({
            ...data,
            authorId: new ObjectId(userId),
            viewCount: 0,
            likeCount: 0,
            createdAt: now,
            updatedAt: now,
        });

        return c.json({ id: result.insertedId.toString(), ...data }, 201);
    } catch (error) {
        if (error instanceof z.ZodError) {
            return c.json({ error: 'Validation failed', details: error.errors }, 400);
        }
        throw error;
    }
});

// Update post
postRoutes.put('/:id', async (c) => {
    try {
        const { id } = c.req.param();
        const userId = c.get('userId');
        const body = await c.req.json();
        const data = updatePostSchema.parse(body);

        if (!ObjectId.isValid(id)) {
            return c.json({ error: 'Invalid post ID' }, 400);
        }

        const posts = getCollection<Post>('posts');

        // Check ownership
        const post = await posts.findOne({ _id: new ObjectId(id) });
        if (!post) {
            return c.json({ error: 'Post not found' }, 404);
        }

        if (post.authorId.toString() !== userId) {
            return c.json({ error: 'Not authorized' }, 403);
        }

        const result = await posts.updateOne(
            { _id: new ObjectId(id) },
            {
                $set: {
                    ...data,
                    updatedAt: new Date(),
                },
            }
        );

        return c.json({ updated: result.modifiedCount > 0 });
    } catch (error) {
        if (error instanceof z.ZodError) {
            return c.json({ error: 'Validation failed', details: error.errors }, 400);
        }
        throw error;
    }
});

// Delete post
postRoutes.delete('/:id', async (c) => {
    const { id } = c.req.param();
    const userId = c.get('userId');

    if (!ObjectId.isValid(id)) {
        return c.json({ error: 'Invalid post ID' }, 400);
    }

    const posts = getCollection<Post>('posts');

    // Check ownership
    const post = await posts.findOne({ _id: new ObjectId(id) });
    if (!post) {
        return c.json({ error: 'Post not found' }, 404);
    }

    if (post.authorId.toString() !== userId) {
        return c.json({ error: 'Not authorized' }, 403);
    }

    await posts.deleteOne({ _id: new ObjectId(id) });

    return c.json({ deleted: true });
});

// Like post
postRoutes.post('/:id/like', async (c) => {
    const { id } = c.req.param();

    if (!ObjectId.isValid(id)) {
        return c.json({ error: 'Invalid post ID' }, 400);
    }

    const posts = getCollection<Post>('posts');

    const result = await posts.updateOne(
        { _id: new ObjectId(id) },
        { $inc: { likeCount: 1 } }
    );

    if (result.matchedCount === 0) {
        return c.json({ error: 'Post not found' }, 404);
    }

    return c.json({ liked: true });
});
