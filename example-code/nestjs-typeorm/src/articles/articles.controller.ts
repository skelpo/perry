// Articles controller
import {
    Controller,
    Get,
    Post,
    Put,
    Delete,
    Body,
    Param,
    Query,
    UseGuards,
    Request,
    ParseUUIDPipe,
} from '@nestjs/common';
import { ArticlesService } from './articles.service';
import { CreateArticleDto, UpdateArticleDto } from './dto';
import { JwtAuthGuard } from '../auth/jwt-auth.guard';

@Controller('articles')
export class ArticlesController {
    constructor(private readonly articlesService: ArticlesService) {}

    @Get()
    findAll(
        @Query('page') page?: string,
        @Query('limit') limit?: string,
        @Query('published') published?: string,
        @Query('tag') tag?: string,
        @Query('authorId') authorId?: string,
    ) {
        return this.articlesService.findAll({
            page: page ? parseInt(page, 10) : undefined,
            limit: limit ? parseInt(limit, 10) : undefined,
            published: published === 'true' ? true : published === 'false' ? false : undefined,
            tag,
            authorId,
        });
    }

    @Get(':id')
    findOne(@Param('id', ParseUUIDPipe) id: string) {
        return this.articlesService.findOne(id);
    }

    @Get('slug/:slug')
    findBySlug(@Param('slug') slug: string) {
        return this.articlesService.findBySlug(slug);
    }

    @UseGuards(JwtAuthGuard)
    @Post()
    create(@Body() createArticleDto: CreateArticleDto, @Request() req: any) {
        return this.articlesService.create(createArticleDto, req.user.id);
    }

    @UseGuards(JwtAuthGuard)
    @Put(':id')
    update(
        @Param('id', ParseUUIDPipe) id: string,
        @Body() updateArticleDto: UpdateArticleDto,
        @Request() req: any,
    ) {
        return this.articlesService.update(id, updateArticleDto, req.user.id);
    }

    @UseGuards(JwtAuthGuard)
    @Delete(':id')
    remove(@Param('id', ParseUUIDPipe) id: string, @Request() req: any) {
        return this.articlesService.remove(id, req.user.id);
    }
}
