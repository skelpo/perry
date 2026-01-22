class TokenData {
  public symbol: string;
  public address: string;
  public decimals: number;
  public name: string;

  constructor(
    symbol: string,
    address: string,
    decimals: number,
    name: string
  ) {
    this.symbol = symbol;
    this.address = address;
    this.decimals = decimals;
    this.name = name;
  }
}

function generateTokens(): TokenData[] {
  return [
    new TokenData('WETH', '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2', 18, 'Wrapped Ether'),
    new TokenData('USDC', '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', 6, 'USD Coin'),
  ];
}

const tokens = generateTokens();
for (const token of tokens) {
  console.log(token.symbol + ": " + token.name);
}
console.log("Done");
