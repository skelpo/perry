// MongoDB Connection
import { MongoClient, Db, Collection, ObjectId } from 'mongodb';
import { config } from './config';

let client: MongoClient;
let db: Db;

export async function connectDB(): Promise<void> {
    try {
        client = new MongoClient(config.mongodb.uri);
        await client.connect();
        db = client.db(config.mongodb.database);
        console.log('MongoDB connected');

        // Create indexes
        await createIndexes();
    } catch (error) {
        console.error('MongoDB connection error:', error);
        process.exit(1);
    }
}

async function createIndexes(): Promise<void> {
    // Users indexes
    await db.collection('users').createIndex({ email: 1 }, { unique: true });
    await db.collection('users').createIndex({ username: 1 }, { unique: true });

    // Posts indexes
    await db.collection('posts').createIndex({ authorId: 1 });
    await db.collection('posts').createIndex({ createdAt: -1 });
    await db.collection('posts').createIndex({ tags: 1 });
    await db.collection('posts').createIndex(
        { title: 'text', content: 'text' },
        { weights: { title: 10, content: 1 } }
    );

    // Comments indexes
    await db.collection('comments').createIndex({ postId: 1 });
    await db.collection('comments').createIndex({ authorId: 1 });
}

export function getDB(): Db {
    if (!db) {
        throw new Error('Database not initialized');
    }
    return db;
}

export function getCollection<T extends Document>(name: string): Collection<T> {
    return getDB().collection<T>(name);
}

export async function closeDB(): Promise<void> {
    if (client) {
        await client.close();
    }
}

// Document types
export interface User {
    _id?: ObjectId;
    username: string;
    email: string;
    passwordHash: string;
    displayName: string;
    bio?: string;
    createdAt: Date;
    updatedAt: Date;
}

export interface Post {
    _id?: ObjectId;
    title: string;
    content: string;
    authorId: ObjectId;
    tags: string[];
    published: boolean;
    viewCount: number;
    likeCount: number;
    createdAt: Date;
    updatedAt: Date;
}

export interface Comment {
    _id?: ObjectId;
    postId: ObjectId;
    authorId: ObjectId;
    content: string;
    parentId?: ObjectId;
    createdAt: Date;
    updatedAt: Date;
}
