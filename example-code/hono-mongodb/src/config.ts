// Configuration
import dotenv from 'dotenv';

dotenv.config();

export const config = {
    port: parseInt(process.env.PORT || '3000', 10),
    nodeEnv: process.env.NODE_ENV || 'development',

    mongodb: {
        uri: process.env.MONGODB_URI || 'mongodb://localhost:27017',
        database: process.env.MONGODB_DATABASE || 'honoapp',
    },

    jwt: {
        secret: process.env.JWT_SECRET || 'your-super-secret-key',
        expiresIn: '7d',
    },

    bcrypt: {
        saltRounds: 10,
    },
};
