// Articles service
import { Injectable, NotFoundException, ForbiddenException } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { Article } from './article.entity';
import { CreateArticleDto, UpdateArticleDto } from './dto';

@Injectable()
export class ArticlesService {
    constructor(
        @InjectRepository(Article)
        private articlesRepository: Repository<Article>,
    ) {}

    async findAll(options: {
        page?: number;
        limit?: number;
        published?: boolean;
        tag?: string;
        authorId?: string;
    } = {}): Promise<{ data: Article[]; total: number }> {
        const { page = 1, limit = 10, published, tag, authorId } = options;

        const queryBuilder = this.articlesRepository
            .createQueryBuilder('article')
            .leftJoinAndSelect('article.author', 'author')
            .select([
                'article.id',
                'article.title',
                'article.slug',
                'article.excerpt',
                'article.tags',
                'article.published',
                'article.viewCount',
                'article.createdAt',
                'author.id',
                'author.username',
                'author.displayName',
                'author.avatar',
            ]);

        if (published !== undefined) {
            queryBuilder.andWhere('article.published = :published', { published });
        }

        if (tag) {
            queryBuilder.andWhere('article.tags LIKE :tag', { tag: `%${tag}%` });
        }

        if (authorId) {
            queryBuilder.andWhere('article.authorId = :authorId', { authorId });
        }

        const [data, total] = await queryBuilder
            .orderBy('article.createdAt', 'DESC')
            .skip((page - 1) * limit)
            .take(limit)
            .getManyAndCount();

        return { data, total };
    }

    async findOne(id: string): Promise<Article> {
        const article = await this.articlesRepository.findOne({
            where: { id },
            relations: ['author'],
        });

        if (!article) {
            throw new NotFoundException('Article not found');
        }

        return article;
    }

    async findBySlug(slug: string): Promise<Article> {
        const article = await this.articlesRepository.findOne({
            where: { slug },
            relations: ['author'],
        });

        if (!article) {
            throw new NotFoundException('Article not found');
        }

        // Increment view count
        await this.articlesRepository.increment({ id: article.id }, 'viewCount', 1);

        return article;
    }

    async create(createArticleDto: CreateArticleDto, authorId: string): Promise<Article> {
        const slug = this.generateSlug(createArticleDto.title);

        const article = this.articlesRepository.create({
            ...createArticleDto,
            slug,
            authorId,
        });

        return this.articlesRepository.save(article);
    }

    async update(
        id: string,
        updateArticleDto: UpdateArticleDto,
        userId: string,
    ): Promise<Article> {
        const article = await this.findOne(id);

        if (article.authorId !== userId) {
            throw new ForbiddenException('You can only edit your own articles');
        }

        if (updateArticleDto.title) {
            updateArticleDto.slug = this.generateSlug(updateArticleDto.title);
        }

        Object.assign(article, updateArticleDto);
        return this.articlesRepository.save(article);
    }

    async remove(id: string, userId: string): Promise<void> {
        const article = await this.findOne(id);

        if (article.authorId !== userId) {
            throw new ForbiddenException('You can only delete your own articles');
        }

        await this.articlesRepository.remove(article);
    }

    private generateSlug(title: string): string {
        return title
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, '-')
            .replace(/^-|-$/g, '')
            + '-' + Date.now().toString(36);
    }
}
