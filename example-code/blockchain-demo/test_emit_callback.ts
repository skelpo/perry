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

const inner = new Inner('ethereum');
inner.on('event', (event: ProcessedEvent) => {
  console.log("Got event!");
});
inner.connect();
console.log("Done");
