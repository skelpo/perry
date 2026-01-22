import { EventEmitter } from 'events';

interface ProcessedEvent {
  network: string;
}

class Inner extends EventEmitter {
  private network: string;

  constructor(network: string) {
    super();
    this.network = network;
  }

  public connect(): void {
    const event: ProcessedEvent = {
      network: this.network
    };
    this.emit('event', event);
  }
}

class Outer extends EventEmitter {
  public initialize(): void {
    const connection = new Inner('ethereum');

    connection.on('event', (event: ProcessedEvent) => {
      console.log("Inner got event");
      this.emit('event', event);  // Forward the event
    });

    connection.connect();
  }
}

const outer = new Outer();
outer.initialize();
console.log("Done");
