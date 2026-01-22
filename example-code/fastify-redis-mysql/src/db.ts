// MySQL Database Connection
import mysql from 'mysql2/promise';
import { config } from './config';

export function createPool() {
    return mysql.createPool({
        host: config.mysql.host,
        port: config.mysql.port,
        database: config.mysql.database,
        user: config.mysql.user,
        password: config.mysql.password,
        connectionLimit: config.mysql.connectionLimit,
        waitForConnections: true,
        queueLimit: 0,
    });
}

export type Pool = ReturnType<typeof createPool>;
