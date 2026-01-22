import { EventEmitter } from 'events';

interface ProcessedEvent {
  network: string;
  timestamp: number;
}

class Connection extends EventEmitter {
  private network: string;

  constructor(network: string) {
    super();
    this.network = network;
  }

  public connect(): void {
    const event: ProcessedEvent = {
      network: this.network,
      timestamp: Date.now()
    };
    this.emit('event', event);
  }
}

class Manager extends EventEmitter {
  public initialize(): void {
    const networks = ['ethereum', 'polygon'];

    for (const network of networks) {
      const connection = new Connection(network);

      connection.on('event', (event: ProcessedEvent) => {
        this.emit('event', event);
      });

      connection.connect();
    }
  }
}

const manager = new Manager();
manager.on('event', (event: ProcessedEvent) => {
  console.log("Received event");
});
manager.initialize();
console.log("Done");
