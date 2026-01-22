// Comment routes
import { Hono } from 'hono';
import { z } from 'zod';
import { ObjectId } from 'mongodb';
import { getCollection, Comment, User, Post } from '../db';

export const commentRoutes = new Hono();

const createCommentSchema = z.object({
    postId: z.string(),
    content: z.string().min(1).max(2000),
    parentId: z.string().optional(),
});

const updateCommentSchema = z.object({
    content: z.string().min(1).max(2000),
});

// Get comments for a post
commentRoutes.get('/post/:postId', async (c) => {
    const { postId } = c.req.param();

    if (!ObjectId.isValid(postId)) {
        return c.json({ error: 'Invalid post ID' }, 400);
    }

    const comments = getCollection<Comment>('comments');
    const results = await comments
        .find({ postId: new ObjectId(postId), parentId: { $exists: false } })
        .sort({ createdAt: -1 })
        .toArray();

    // Get replies for each comment
    const commentIds = results.map(c => c._id!);
    const replies = await comments
        .find({ parentId: { $in: commentIds } })
        .sort({ createdAt: 1 })
        .toArray();

    // Get all author info
    const allComments = [...results, ...replies];
    const authorIds = [...new Set(allComments.map(c => c.authorId.toString()))];
    const users = getCollection<User>('users');
    const authors = await users
        .find({ _id: { $in: authorIds.map(id => new ObjectId(id)) } })
        .project({ passwordHash: 0 })
        .toArray();

    const authorMap = new Map(authors.map(a => [a._id!.toString(), a]));

    // Build nested structure
    const replyMap = new Map<string, Comment[]>();
    for (const reply of replies) {
        const parentIdStr = reply.parentId!.toString();
        if (!replyMap.has(parentIdStr)) {
            replyMap.set(parentIdStr, []);
        }
        replyMap.get(parentIdStr)!.push(reply);
    }

    const commentsWithReplies = results.map(comment => ({
        ...comment,
        author: authorMap.get(comment.authorId.toString()),
        replies: (replyMap.get(comment._id!.toString()) || []).map(reply => ({
            ...reply,
            author: authorMap.get(reply.authorId.toString()),
        })),
    }));

    return c.json(commentsWithReplies);
});

// Create comment
commentRoutes.post('/', async (c) => {
    try {
        const userId = c.get('userId');
        const body = await c.req.json();
        const data = createCommentSchema.parse(body);

        if (!ObjectId.isValid(data.postId)) {
            return c.json({ error: 'Invalid post ID' }, 400);
        }

        if (data.parentId && !ObjectId.isValid(data.parentId)) {
            return c.json({ error: 'Invalid parent comment ID' }, 400);
        }

        // Verify post exists
        const posts = getCollection<Post>('posts');
        const post = await posts.findOne({ _id: new ObjectId(data.postId) });
        if (!post) {
            return c.json({ error: 'Post not found' }, 404);
        }

        // Verify parent comment exists (if provided)
        if (data.parentId) {
            const comments = getCollection<Comment>('comments');
            const parentComment = await comments.findOne({ _id: new ObjectId(data.parentId) });
            if (!parentComment) {
                return c.json({ error: 'Parent comment not found' }, 404);
            }
        }

        const comments = getCollection<Comment>('comments');
        const now = new Date();

        const result = await comments.insertOne({
            postId: new ObjectId(data.postId),
            authorId: new ObjectId(userId),
            content: data.content,
            parentId: data.parentId ? new ObjectId(data.parentId) : undefined,
            createdAt: now,
            updatedAt: now,
        });

        return c.json({
            id: result.insertedId.toString(),
            content: data.content,
        }, 201);
    } catch (error) {
        if (error instanceof z.ZodError) {
            return c.json({ error: 'Validation failed', details: error.errors }, 400);
        }
        throw error;
    }
});

// Update comment
commentRoutes.put('/:id', async (c) => {
    try {
        const { id } = c.req.param();
        const userId = c.get('userId');
        const body = await c.req.json();
        const data = updateCommentSchema.parse(body);

        if (!ObjectId.isValid(id)) {
            return c.json({ error: 'Invalid comment ID' }, 400);
        }

        const comments = getCollection<Comment>('comments');

        // Check ownership
        const comment = await comments.findOne({ _id: new ObjectId(id) });
        if (!comment) {
            return c.json({ error: 'Comment not found' }, 404);
        }

        if (comment.authorId.toString() !== userId) {
            return c.json({ error: 'Not authorized' }, 403);
        }

        await comments.updateOne(
            { _id: new ObjectId(id) },
            {
                $set: {
                    content: data.content,
                    updatedAt: new Date(),
                },
            }
        );

        return c.json({ updated: true });
    } catch (error) {
        if (error instanceof z.ZodError) {
            return c.json({ error: 'Validation failed', details: error.errors }, 400);
        }
        throw error;
    }
});

// Delete comment
commentRoutes.delete('/:id', async (c) => {
    const { id } = c.req.param();
    const userId = c.get('userId');

    if (!ObjectId.isValid(id)) {
        return c.json({ error: 'Invalid comment ID' }, 400);
    }

    const comments = getCollection<Comment>('comments');

    // Check ownership
    const comment = await comments.findOne({ _id: new ObjectId(id) });
    if (!comment) {
        return c.json({ error: 'Comment not found' }, 404);
    }

    if (comment.authorId.toString() !== userId) {
        return c.json({ error: 'Not authorized' }, 403);
    }

    // Delete comment and all replies
    await comments.deleteMany({
        $or: [
            { _id: new ObjectId(id) },
            { parentId: new ObjectId(id) },
        ],
    });

    return c.json({ deleted: true });
});
