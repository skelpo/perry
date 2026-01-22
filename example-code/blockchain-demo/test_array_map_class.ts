import { EventEmitter } from 'events';

type NetworkName = 'ethereum' | 'polygon';

class Connection extends EventEmitter {
  private network: NetworkName;

  constructor(network: NetworkName) {
    super();
    this.network = network;
  }
}

class Manager {
  private connections: Map<NetworkName, Connection> = new Map();

  public initialize(): void {
    const networks: NetworkName[] = ['ethereum'];

    networks.map((network) => {
      const connection = new Connection(network);
      this.connections.set(network, connection);
    });
  }
}

const manager = new Manager();
manager.initialize();
console.log("Done");
