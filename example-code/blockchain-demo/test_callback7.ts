import { EventEmitter } from 'events';

class Inner extends EventEmitter {
  public fire(): void {
    this.emit('test', { data: 'hello' });
  }
}

class Outer extends EventEmitter {
  private inners: Map<string, Inner> = new Map();
  
  public setup(): void {
    const names = ['a', 'b'];
    
    names.map((name) => {
      const inner = new Inner();
      
      inner.on('test', (event) => {
        this.emit('outer', event);
      });
      
      this.inners.set(name, inner);
    });
  }
}

const outer = new Outer();
outer.setup();
console.log("Test");
