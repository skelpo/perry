import Redis from 'ioredis';

async function main() {
    try {
        console.log('Creating Redis connection...');
        const redis = await new Redis();
        console.log('Connection ID:', redis);

        console.log('Setting value...');
        const setResult = await redis.set('perry-test', 'hello');
        console.log('Set result:', setResult);

        console.log('Getting value...');
        const value = await redis.get('perry-test');
        console.log('Get result:', value);
        console.log('Redis value:', value);

        console.log('Cleaning up...');
        await redis.del('perry-test');
        await redis.quit();
        console.log('Done');
    } catch (e) {
        console.log('Error:', e);
    }
}

main();
