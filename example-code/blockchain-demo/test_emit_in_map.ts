import { EventEmitter } from 'events';

type NetworkName = 'ethereum' | 'polygon';

interface ProcessedEvent {
  network: NetworkName;
  timestamp: number;
}

class Connection extends EventEmitter {
  private network: NetworkName;

  constructor(network: NetworkName) {
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
  private connections: Map<NetworkName, Connection> = new Map();

  public initialize(): void {
    const networks: NetworkName[] = ['ethereum'];

    networks.map((network) => {
      const connection = new Connection(network);

      connection.on('event', (event: ProcessedEvent) => {
        this.emit('event', event);
      });

      connection.connect();
      this.connections.set(network, connection);
    });
  }
}

const manager = new Manager();
manager.initialize();
console.log("Done");
