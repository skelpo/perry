import { EventEmitter } from 'events';

interface ProcessedEvent {
  network: string;
  timestamp: number;
}

class MyEmitter extends EventEmitter {
  private network: string;

  constructor(network: string) {
    super();
    this.network = network;
  }

  public async connect(): Promise<void> {
    await new Promise(resolve => setTimeout(resolve, 10));
    const event: ProcessedEvent = {
      network: this.network,
      timestamp: Date.now()
    };
    this.emit('event', event);
  }
}

class Manager extends EventEmitter {
  public async initialize(): Promise<void> {
    const networks = ['ethereum'];

    const promises = networks.map(async (network) => {
      const connection = new MyEmitter(network);

      connection.on('event', (event: ProcessedEvent) => {
        this.emit('event', event);
      });

      await connection.connect();
    });

    await Promise.all(promises);
  }
}

const manager = new Manager();
manager.initialize().then(() => {
  console.log("Done");
});
