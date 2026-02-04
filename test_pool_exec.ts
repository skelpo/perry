import mysql from 'mysql2/promise';

async function test() {
    const pool = mysql.createPool({ uri: 'mysql://localhost/test' });
    console.log('typeof pool:', typeof pool);

    // Try to call execute
    const result = await pool.execute('SELECT 1');
    console.log('result:', result);
}

test();
