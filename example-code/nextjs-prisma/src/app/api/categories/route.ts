// Categories API route
import { NextRequest, NextResponse } from 'next/server';
import { z } from 'zod';
import { prisma } from '@/lib/prisma';

const createCategorySchema = z.object({
    name: z.string().min(1).max(50),
    slug: z.string().min(1).max(50).regex(/^[a-z0-9-]+$/),
});

// GET /api/categories
export async function GET() {
    const categories = await prisma.category.findMany({
        include: {
            _count: {
                select: { posts: true },
            },
        },
        orderBy: { name: 'asc' },
    });

    return NextResponse.json(categories);
}

// POST /api/categories
export async function POST(request: NextRequest) {
    try {
        const body = await request.json();
        const data = createCategorySchema.parse(body);

        const existing = await prisma.category.findFirst({
            where: {
                OR: [
                    { name: data.name },
                    { slug: data.slug },
                ],
            },
        });

        if (existing) {
            return NextResponse.json(
                { error: 'Category already exists' },
                { status: 409 }
            );
        }

        const category = await prisma.category.create({
            data: {
                name: data.name,
                slug: data.slug,
            },
        });

        return NextResponse.json(category, { status: 201 });
    } catch (error) {
        if (error instanceof z.ZodError) {
            return NextResponse.json(
                { error: 'Validation failed', details: error.errors },
                { status: 400 }
            );
        }
        console.error('Create category error:', error);
        return NextResponse.json({ error: 'Failed to create category' }, { status: 500 });
    }
}
