/**
 * Simple module with exported functions
 */
export function add(a: number, b: number): number {
  return a + b;
}

export async function greet(name: string): Promise<string> {
  return 'Hello, ' + name;
}
