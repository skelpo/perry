// Article DTOs
import { IsString, IsBoolean, IsOptional, IsArray, MinLength, MaxLength } from 'class-validator';

export class CreateArticleDto {
    @IsString()
    @MinLength(1)
    @MaxLength(200)
    title: string;

    @IsString()
    @MinLength(1)
    content: string;

    @IsString()
    @IsOptional()
    excerpt?: string;

    @IsBoolean()
    @IsOptional()
    published?: boolean;

    @IsArray()
    @IsString({ each: true })
    @IsOptional()
    tags?: string[];
}

export class UpdateArticleDto {
    @IsString()
    @MinLength(1)
    @MaxLength(200)
    @IsOptional()
    title?: string;

    @IsString()
    @IsOptional()
    slug?: string;

    @IsString()
    @MinLength(1)
    @IsOptional()
    content?: string;

    @IsString()
    @IsOptional()
    excerpt?: string;

    @IsBoolean()
    @IsOptional()
    published?: boolean;

    @IsArray()
    @IsString({ each: true })
    @IsOptional()
    tags?: string[];
}
