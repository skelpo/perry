import { EventEmitter } from 'events';
import { LRUCache } from 'lru-cache';

type NetworkName = 'ethereum' | 'polygon';

class PoolData {
  public id: number;
  constructor() {
    this.id = 0;
  }
}

// Using LRUCache
class DemoCacheManager {
  private poolCache: LRUCache<string, PoolData>;

  constructor(cacheSize: number) {
    this.poolCache = new LRUCache<string, PoolData>({
      max: cacheSize,
      ttl: 60000,
    });
  }

  public getPool(address: string): PoolData | undefined {
    return this.poolCache.get(address);
  }
}

const cache = new DemoCacheManager(100);
console.log("Cache created");
