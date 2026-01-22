import { EventEmitter } from 'events';

class Inner extends EventEmitter {
  public doSomething(): void {
    console.log("Inner doSomething");
  }
}

class Outer extends EventEmitter {
  public initialize(): void {
    const networks = ['ethereum'];
    
    networks.map((network) => {
      const connection = new Inner();
      
      connection.on('event', (event) => {
        this.emit('event', event);  // Captures 'this'
      });
      
      connection.doSomething();  // This line triggers the bug!
    });
  }
}

const outer = new Outer();
outer.initialize();
console.log("Test");
