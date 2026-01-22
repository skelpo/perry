import { EventEmitter } from 'events';
import { ethers } from 'ethers';
import WebSocket from 'ws';
import Redis from 'ioredis';
import { LRUCache } from 'lru-cache';
import { createPublicClient, http, parseAbiItem } from 'viem';
import { mainnet } from 'viem/chains';
import Big from 'big.js';
import Decimal from 'decimal.js-light';
import JSBI from 'jsbi';
import invariant from 'tiny-invariant';
import { backOff } from 'exponential-backoff';
import { Command } from 'commander';
import 'dotenv/config';

// Type definitions
type NetworkName = 'ethereum' | 'polygon' | 'arbitrum' | 'optimism' | 'base';
type TierType = 'premium' | 'standard';

// Simple class
class TokenData {
  public symbol: string;
  public address: string;
  public decimals: number;
  public name: string;
  public id: number | null;

  constructor(
    symbol: string,
    address: string,
    decimals: number,
    name: string,
    id: number | null = null
  ) {
    this.symbol = symbol;
    this.address = address;
    this.decimals = decimals;
    this.name = name;
    this.id = id;
  }
}

const token = new TokenData('WETH', '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2', 18, 'Wrapped Ether', 1);
console.log(token.symbol);
