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
    const connection = new MyEmitter('ethereum');

    // The closure receives an object parameter
    connection.on('event', (event: ProcessedEvent) => {
      // Forward the event - this closure is called with event as parameter
      this.emit('event', event);
    });

    connection.connect();
  }
}

const manager = new Manager();
manager.initialize();
console.log("Done");
