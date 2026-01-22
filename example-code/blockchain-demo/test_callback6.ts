import { EventEmitter } from 'events';

class Inner extends EventEmitter {
  public fire(): void {
    this.emit('test', { data: 'hello' });
  }
}

class Outer extends EventEmitter {
  public setup(): void {
    const inner = new Inner();
    
    inner.on('test', (event) => {
      this.emit('outer', event);
    });
    
    inner.fire();
  }
}

const outer = new Outer();
outer.setup();
console.log("Test");
