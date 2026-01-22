// Article entity
import {
    Entity,
    PrimaryGeneratedColumn,
    Column,
    CreateDateColumn,
    UpdateDateColumn,
    ManyToOne,
    JoinColumn,
} from 'typeorm';
import { User } from '../users/user.entity';

@Entity('articles')
export class Article {
    @PrimaryGeneratedColumn('uuid')
    id: string;

    @Column()
    title: string;

    @Column({ unique: true })
    slug: string;

    @Column('text')
    content: string;

    @Column({ nullable: true })
    excerpt: string;

    @Column({ default: false })
    published: boolean;

    @Column({ type: 'simple-array', nullable: true })
    tags: string[];

    @Column({ default: 0 })
    viewCount: number;

    @ManyToOne(() => User, user => user.articles)
    @JoinColumn({ name: 'authorId' })
    author: User;

    @Column()
    authorId: string;

    @CreateDateColumn()
    createdAt: Date;

    @UpdateDateColumn()
    updatedAt: Date;
}
