// Users controller
import {
    Controller,
    Get,
    Put,
    Delete,
    Body,
    Param,
    UseGuards,
    Request,
    ParseUUIDPipe,
} from '@nestjs/common';
import { UsersService } from './users.service';
import { UpdateUserDto } from './dto';
import { JwtAuthGuard } from '../auth/jwt-auth.guard';

@Controller('users')
export class UsersController {
    constructor(private readonly usersService: UsersService) {}

    @Get()
    findAll() {
        return this.usersService.findAll();
    }

    @Get(':id')
    findOne(@Param('id', ParseUUIDPipe) id: string) {
        return this.usersService.findOne(id);
    }

    @UseGuards(JwtAuthGuard)
    @Put('me')
    updateMe(@Request() req: any, @Body() updateUserDto: UpdateUserDto) {
        return this.usersService.update(req.user.id, updateUserDto);
    }

    @UseGuards(JwtAuthGuard)
    @Delete('me')
    deleteMe(@Request() req: any) {
        return this.usersService.remove(req.user.id);
    }
}
