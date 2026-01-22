// User DTOs
import { IsEmail, IsString, MinLength, MaxLength, IsOptional } from 'class-validator';

export class CreateUserDto {
    @IsEmail()
    email: string;

    @IsString()
    @MinLength(3)
    @MaxLength(30)
    username: string;

    @IsString()
    @MinLength(8)
    password: string;

    @IsString()
    @IsOptional()
    displayName?: string;
}

export class UpdateUserDto {
    @IsString()
    @IsOptional()
    @MinLength(8)
    password?: string;

    @IsString()
    @IsOptional()
    displayName?: string;

    @IsString()
    @IsOptional()
    bio?: string;

    @IsString()
    @IsOptional()
    avatar?: string;
}

export class LoginDto {
    @IsEmail()
    email: string;

    @IsString()
    password: string;
}
