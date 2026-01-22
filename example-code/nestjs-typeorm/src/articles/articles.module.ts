// Articles module
import { Module } from '@nestjs/common';
import { TypeOrmModule } from '@nestjs/typeorm';
import { Article } from './article.entity';
import { ArticlesService } from './articles.service';
import { ArticlesController } from './articles.controller';

@Module({
    imports: [TypeOrmModule.forFeature([Article])],
    providers: [ArticlesService],
    controllers: [ArticlesController],
})
export class ArticlesModule {}
