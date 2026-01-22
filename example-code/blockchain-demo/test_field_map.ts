import { EventEmitter } from 'events';

type NetworkName = 'ethereum' | 'polygon';

interface Config {
  networks: NetworkName[];
}

class Manager extends EventEmitter {
  private config: Config;

  constructor(config: Config) {
    super();
    this.config = config;
  }

  public initialize(): void {
    this.config.networks.map((network) => {
      console.log('Network: ' + network);
    });
  }
}

const config: Config = {
  networks: ['ethereum'],
};

const manager = new Manager(config);
manager.initialize();
console.log("Done");
